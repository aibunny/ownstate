//! Deterministic candidate → canonical promotion rules.
//!
//! This is the ONLY path by which anything becomes canonical knowledge.
//! Extraction models and MCP clients can merely create candidates; promotion
//! is decided by this pure, unit-testable function and executed
//! transactionally by the storage layer.

use crate::entities::{CandidateKnowledge, KnowledgeVersion};
use crate::enums::{CandidateStatus, KnowledgeStatus, ProposerKind, TrustLevel};
use crate::error::DomainError;
use crate::ids::KnowledgeVersionId;
use crate::limits;

/// The decision produced by the promotion rules. The storage layer applies it
/// atomically: supersede the old ACTIVE version (if any), insert the new one.
#[derive(Debug, Clone, PartialEq)]
pub struct PromotionDecision {
    pub trust_level: TrustLevel,
    pub version_number: i32,
    pub supersedes_version_id: Option<KnowledgeVersionId>,
    pub initial_status: KnowledgeStatus,
}

/// Decide whether and how a candidate becomes a canonical knowledge version.
///
/// Rules:
/// - Only PENDING candidates can be promoted.
/// - Content and subject key must satisfy size limits.
/// - Trust is derived from the proposer and provenance, never taken from the
///   proposal itself:
///   - HUMAN            → HUMAN_EXPLICIT
///   - AGENT/EXTRACTOR with evidence   → AGENT_DERIVED
///   - AGENT/EXTRACTOR without evidence → EXTERNAL_UNTRUSTED
/// - If the item already has an ACTIVE version, the new version supersedes it
///   (history preserved); otherwise this becomes version 1.
pub fn decide_promotion(
    candidate: &CandidateKnowledge,
    existing_active: Option<&KnowledgeVersion>,
    current_max_version: i32,
) -> Result<PromotionDecision, DomainError> {
    if candidate.status != CandidateStatus::Pending {
        return Err(DomainError::CandidateNotPending(
            candidate.status.as_str().to_string(),
        ));
    }
    if candidate.content.trim().is_empty() {
        return Err(DomainError::validation("candidate content is empty"));
    }
    if candidate.content.len() > limits::MAX_CONTENT_BYTES {
        return Err(DomainError::validation(format!(
            "candidate content exceeds {} bytes",
            limits::MAX_CONTENT_BYTES
        )));
    }
    if candidate.subject_key.trim().is_empty()
        || candidate.subject_key.len() > limits::MAX_SUBJECT_KEY_LEN
    {
        return Err(DomainError::validation("invalid subject_key length"));
    }
    if let Some(c) = candidate.confidence
        && !(0.0..=1.0).contains(&c)
    {
        return Err(DomainError::validation("confidence must be within [0, 1]"));
    }
    if let Some(active) = existing_active {
        // Defensive: the storage layer must hand us the ACTIVE version of the
        // *same* item the candidate maps onto.
        if active.status != KnowledgeStatus::Active {
            return Err(DomainError::validation(
                "existing version passed to promotion is not ACTIVE",
            ));
        }
    }

    let trust_level = match candidate.proposed_by {
        ProposerKind::Human => TrustLevel::HumanExplicit,
        ProposerKind::Agent | ProposerKind::Extractor => {
            if candidate.evidence_event_ids.is_empty() {
                TrustLevel::ExternalUntrusted
            } else {
                TrustLevel::AgentDerived
            }
        }
    };

    Ok(PromotionDecision {
        trust_level,
        version_number: current_max_version
            .checked_add(1)
            .ok_or_else(|| DomainError::validation("version number overflow"))?,
        supersedes_version_id: existing_active.map(|v| v.id),
        initial_status: KnowledgeStatus::Active,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use chrono::Utc;
    use serde_json::json;

    use super::*;
    use crate::enums::{KnowledgeKind, SecurityClassification};
    use crate::ids::*;

    fn candidate(proposed_by: ProposerKind, evidence: usize) -> CandidateKnowledge {
        CandidateKnowledge {
            id: CandidateId::generate(),
            tenant_id: TenantId::generate(),
            project_id: ProjectId::generate(),
            session_id: None,
            kind: KnowledgeKind::Architecture,
            subject_key: "authentication".into(),
            content: "Auth flows through middleware then handler.".into(),
            structured_content: Some(json!({"components": ["middleware", "handler"]})),
            confidence: Some(0.9),
            proposed_by,
            source: Some("test".into()),
            evidence_event_ids: (0..evidence).map(|_| EventId::generate()).collect(),
            security_classification: SecurityClassification::Internal,
            status: CandidateStatus::Pending,
            rejection_reason: None,
            promoted_version_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn active_version(number: i32) -> KnowledgeVersion {
        KnowledgeVersion {
            id: KnowledgeVersionId::generate(),
            knowledge_item_id: KnowledgeItemId::generate(),
            version_number: number,
            content: "old".into(),
            structured_content: None,
            status: KnowledgeStatus::Active,
            trust_level: TrustLevel::AgentDerived,
            confidence: None,
            security_classification: SecurityClassification::Internal,
            valid_from: Utc::now(),
            valid_until: None,
            supersedes_version_id: None,
            candidate_id: None,
            created_at: Utc::now(),
            created_by: "test".into(),
        }
    }

    #[test]
    fn human_proposals_get_human_explicit_trust() {
        let d = decide_promotion(&candidate(ProposerKind::Human, 0), None, 0).unwrap();
        assert_eq!(d.trust_level, TrustLevel::HumanExplicit);
        assert_eq!(d.version_number, 1);
        assert_eq!(d.supersedes_version_id, None);
        assert_eq!(d.initial_status, KnowledgeStatus::Active);
    }

    #[test]
    fn agent_proposals_with_evidence_are_agent_derived() {
        let d = decide_promotion(&candidate(ProposerKind::Agent, 2), None, 0).unwrap();
        assert_eq!(d.trust_level, TrustLevel::AgentDerived);
    }

    #[test]
    fn extractor_proposals_without_evidence_are_untrusted() {
        let d = decide_promotion(&candidate(ProposerKind::Extractor, 0), None, 0).unwrap();
        assert_eq!(d.trust_level, TrustLevel::ExternalUntrusted);
    }

    #[test]
    fn extractor_cannot_bypass_trust_rules() {
        // Whatever confidence an extractor claims, trust derives from
        // provenance, not from the proposal.
        let mut c = candidate(ProposerKind::Extractor, 0);
        c.confidence = Some(1.0);
        let d = decide_promotion(&c, None, 0).unwrap();
        assert_ne!(d.trust_level, TrustLevel::HumanExplicit);
    }

    #[test]
    fn update_supersedes_existing_active_version() {
        let v1 = active_version(1);
        let d = decide_promotion(&candidate(ProposerKind::Agent, 1), Some(&v1), 1).unwrap();
        assert_eq!(d.version_number, 2);
        assert_eq!(d.supersedes_version_id, Some(v1.id));
    }

    #[test]
    fn non_pending_candidates_are_rejected() {
        let mut c = candidate(ProposerKind::Human, 0);
        c.status = CandidateStatus::Promoted;
        assert!(matches!(
            decide_promotion(&c, None, 1),
            Err(DomainError::CandidateNotPending(_))
        ));
    }

    #[test]
    fn empty_content_is_rejected() {
        let mut c = candidate(ProposerKind::Human, 0);
        c.content = "   ".into();
        assert!(decide_promotion(&c, None, 0).is_err());
    }

    #[test]
    fn out_of_range_confidence_is_rejected() {
        let mut c = candidate(ProposerKind::Agent, 1);
        c.confidence = Some(1.5);
        assert!(decide_promotion(&c, None, 0).is_err());
    }
}
