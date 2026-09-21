//! Candidate proposal and deterministic promotion into canonical knowledge.
//!
//! Proposals are untrusted input regardless of origin (HTTP, MCP, future
//! extractors). Promotion is the only path to canonical state and runs the
//! pure rules in `ownstate_domain::promotion` inside one transaction.

use chrono::Utc;
use ownstate_domain::{
    CandidateId, CandidateKnowledge, CandidateStatus, EntityId, EventId, EvidenceId,
    KnowledgeEvidence, KnowledgeItem, KnowledgeItemId, KnowledgeKind, KnowledgeVersion,
    KnowledgeVersionId, ProjectId, ProposerKind, SecurityClassification, SessionId,
    decide_promotion, limits,
};
use ownstate_storage::{
    candidates, events, institutional, jobs, knowledge, projects, query, sessions,
};
use serde_json::Value as Json;
use serde_json::json;

use crate::{AppServices, ServiceError, ServiceResult};

pub struct ProposeKnowledge {
    pub project_id: ProjectId,
    pub session_id: Option<SessionId>,
    pub kind: KnowledgeKind,
    pub subject_key: String,
    pub content: String,
    pub structured_content: Option<Json>,
    pub confidence: Option<f32>,
    pub proposed_by: ProposerKind,
    pub source: Option<String>,
    pub evidence_event_ids: Vec<EventId>,
    pub security_classification: Option<SecurityClassification>,
}

pub struct KnowledgeDetail {
    pub item: KnowledgeItem,
    pub versions: Vec<KnowledgeVersion>,
    pub evidence: Vec<KnowledgeEvidence>,
}

/// Associated entity IDs are typed metadata, never permission grants.
fn associated_entities(content: Option<&Json>) -> ServiceResult<Vec<EntityId>> {
    let Some(value) = content.and_then(|c| c.get("entity_ids")) else {
        return Ok(Vec::new());
    };
    let ids: Vec<EntityId> = serde_json::from_value(value.clone()).map_err(|_| {
        ServiceError::validation("structured_content.entity_ids must be an array of entity UUIDs")
    })?;
    if ids.len() > 100 || ids.iter().collect::<std::collections::HashSet<_>>().len() != ids.len() {
        return Err(ServiceError::validation(
            "entity associations must be unique and at most 100",
        ));
    }
    Ok(ids)
}

impl AppServices {
    async fn associated_entity_floor(
        &self,
        project: ProjectId,
        ids: &[EntityId],
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> ServiceResult<Vec<SecurityClassification>> {
        let allowed = SecurityClassification::allowed_up_to(self.max_classification());
        let mut classifications = Vec::new();
        let as_of = query::snapshot_timestamp(&mut **tx).await?;
        for id in ids {
            let versions = institutional::versions(
                &mut **tx,
                institutional::InstitutionalRead {
                    tenant: self.tenant_id(),
                    project,
                    record_id: Some(id.as_uuid()),
                    allowed: &allowed,
                    as_of,
                    record_type: Some("ENTITY"),
                    endpoint: None,
                    limit: 1,
                },
            )
            .await?;
            let version = versions.first().ok_or_else(|| {
                ServiceError::validation(
                    "associated entity is not an accessible current entity in this project",
                )
            })?;
            classifications.push(version.security_classification);
        }
        Ok(classifications)
    }

    /// Record candidate knowledge. Nothing here becomes canonical: the
    /// candidate is validated, scoped and stored PENDING.
    pub(crate) async fn propose_knowledge(
        &self,
        req: ProposeKnowledge,
    ) -> ServiceResult<CandidateKnowledge> {
        let subject_key = req.subject_key.trim().to_string();
        if subject_key.is_empty() || subject_key.len() > limits::MAX_SUBJECT_KEY_LEN {
            return Err(ServiceError::validation(format!(
                "subject_key must be 1..={} characters",
                limits::MAX_SUBJECT_KEY_LEN
            )));
        }
        if req.content.trim().is_empty() {
            return Err(ServiceError::validation("content is empty"));
        }
        if req.content.len() > limits::MAX_CONTENT_BYTES {
            return Err(ServiceError::validation(format!(
                "content exceeds {} bytes",
                limits::MAX_CONTENT_BYTES
            )));
        }
        if let Some(c) = req.confidence
            && !(0.0..=1.0).contains(&c)
        {
            return Err(ServiceError::validation("confidence must be within [0, 1]"));
        }
        if req.evidence_event_ids.len() > limits::MAX_EVIDENCE_REFS {
            return Err(ServiceError::validation(format!(
                "at most {} evidence references allowed",
                limits::MAX_EVIDENCE_REFS
            )));
        }

        // Authorization scope: project (and session, when given) must exist
        // in-tenant, and the session must belong to the project.
        let project = projects::get(self.pool(), self.tenant_id(), req.project_id).await?;
        if let Some(session_id) = req.session_id {
            let session = sessions::get(self.pool(), self.tenant_id(), session_id).await?;
            if session.project_id != project.id {
                return Err(ServiceError::validation(
                    "session does not belong to the project",
                ));
            }
        }

        // Evidence must reference real events inside this project. A proposal
        // cannot smuggle references to another project's evidence.
        let evidence_events = if req.evidence_event_ids.is_empty() {
            Vec::new()
        } else {
            let found = events::get_scoped_by_ids(
                self.pool(),
                self.tenant_id(),
                project.id,
                &req.evidence_event_ids,
            )
            .await?;
            if found.len() != req.evidence_event_ids.len() {
                return Err(ServiceError::validation(
                    "one or more evidence events do not exist in this project",
                ));
            }
            found
        };
        let associated = associated_entities(req.structured_content.as_ref())?;
        let mut tx = self.pool().begin().await?;
        let entity_classes = self
            .associated_entity_floor(project.id, &associated, &mut tx)
            .await?;
        let security_classification = SecurityClassification::with_evidence_floor(
            req.security_classification,
            evidence_events
                .iter()
                .map(|e| e.security_classification)
                .chain(entity_classes),
        );
        if !associated.is_empty() && security_classification > self.max_classification() {
            return Err(ServiceError::validation(
                "associated knowledge exceeds authorized classification",
            ));
        }

        let now = Utc::now();
        let candidate = CandidateKnowledge {
            id: CandidateId::generate(),
            tenant_id: self.tenant_id(),
            project_id: project.id,
            session_id: req.session_id,
            kind: req.kind,
            subject_key,
            content: req.content,
            structured_content: req.structured_content,
            confidence: req.confidence,
            proposed_by: req.proposed_by,
            source: req.source,
            evidence_event_ids: req.evidence_event_ids,
            security_classification,
            status: CandidateStatus::Pending,
            rejection_reason: None,
            promoted_version_id: None,
            created_at: now,
            updated_at: now,
        };
        candidates::insert(&mut *tx, &candidate).await?;
        tx.commit().await?;
        tracing::info!(
            candidate_id = %candidate.id,
            project_id = %project.id,
            kind = %candidate.kind,
            proposed_by = %candidate.proposed_by,
            "candidate knowledge proposed"
        );
        Ok(candidate)
    }

    pub(crate) async fn get_candidate(&self, id: CandidateId) -> ServiceResult<CandidateKnowledge> {
        Ok(candidates::get(self.pool(), self.tenant_id(), id).await?)
    }

    /// Promote a PENDING candidate into canonical knowledge, superseding any
    /// existing ACTIVE version of the same (project, kind, subject). Runs the
    /// deterministic domain rules in a single transaction; enqueues an
    /// embedding job for the new version.
    pub(crate) async fn promote_candidate(
        &self,
        candidate_id: CandidateId,
    ) -> ServiceResult<(KnowledgeItem, KnowledgeVersion)> {
        let mut tx = self.pool().begin().await?;

        let candidate = candidates::get_for_update(&mut tx, self.tenant_id(), candidate_id).await?;
        let project = projects::get(&mut *tx, self.tenant_id(), candidate.project_id).await?;

        // Re-derive the evidence floor inside the promotion transaction. This
        // also protects candidates recorded before classification inheritance
        // was introduced. Source event content/classification is append-only.
        let evidence_events = events::get_scoped_by_ids(
            &mut *tx,
            self.tenant_id(),
            candidate.project_id,
            &candidate.evidence_event_ids,
        )
        .await?;
        if evidence_events.len() != candidate.evidence_event_ids.len() {
            return Err(ServiceError::validation(
                "candidate evidence no longer resolves inside the project",
            ));
        }
        let associated = associated_entities(candidate.structured_content.as_ref())?;
        let entity_classes = self
            .associated_entity_floor(candidate.project_id, &associated, &mut tx)
            .await?;
        let security_classification = SecurityClassification::with_evidence_floor(
            Some(candidate.security_classification),
            evidence_events
                .iter()
                .map(|e| e.security_classification)
                .chain(entity_classes),
        );
        if !associated.is_empty() && security_classification > self.max_classification() {
            return Err(ServiceError::validation(
                "associated knowledge exceeds authorized classification",
            ));
        }

        let existing_item = knowledge::find_item_for_update(
            &mut tx,
            self.tenant_id(),
            candidate.project_id,
            candidate.kind,
            &candidate.subject_key,
        )
        .await?;

        let (existing_active, max_version) = match &existing_item {
            Some(item) => (
                knowledge::get_active_version(&mut tx, item.id).await?,
                knowledge::max_version_number(&mut tx, item.id).await?,
            ),
            None => (None, 0),
        };

        let decision = decide_promotion(&candidate, existing_active.as_ref(), max_version)?;

        let item = match existing_item {
            Some(item) => item,
            None => {
                let item = KnowledgeItem {
                    id: KnowledgeItemId::generate(),
                    tenant_id: self.tenant_id(),
                    project_id: candidate.project_id,
                    // Knowledge inherits the project's ownership domain;
                    // personal knowledge never silently becomes
                    // organizational (or vice versa).
                    ownership_domain: project.ownership_domain,
                    kind: candidate.kind,
                    subject_key: candidate.subject_key.clone(),
                    created_at: Utc::now(),
                    created_by: format!("promotion:{}", candidate.proposed_by.as_str()),
                };
                match knowledge::insert_item(&mut *tx, &item).await {
                    Ok(()) => {}
                    // Two first-promotions for the same (project, kind,
                    // subject) can race past the (absent) row lock; the loser
                    // hits the UNIQUE triple. The candidate stays PENDING —
                    // surface a retryable conflict, not an internal error.
                    Err(e) if e.is_unique_violation() => {
                        return Err(ServiceError::Conflict(
                            "a concurrent promotion created this knowledge item; retry".into(),
                        ));
                    }
                    Err(e) => return Err(e.into()),
                }
                item
            }
        };

        let recorded_at = knowledge::promotion_timestamp(&mut tx).await?;
        // A legacy/future ACTIVE start bounds the validity transition without
        // changing when this new version was actually recorded.
        let transition_at = existing_active
            .as_ref()
            .map_or(recorded_at, |v| recorded_at.max(v.valid_from));
        if let Some(superseded) = decision.supersedes_version_id {
            knowledge::supersede_version_at(&mut tx, superseded, transition_at).await?;
        }

        let version = KnowledgeVersion {
            id: KnowledgeVersionId::generate(),
            knowledge_item_id: item.id,
            version_number: decision.version_number,
            content: candidate.content.clone(),
            structured_content: candidate.structured_content.clone(),
            status: decision.initial_status,
            trust_level: decision.trust_level,
            confidence: candidate.confidence,
            security_classification,
            valid_from: transition_at,
            valid_until: None,
            supersedes_version_id: decision.supersedes_version_id,
            candidate_id: Some(candidate.id),
            created_at: recorded_at,
            created_by: format!("promotion:{}", candidate.proposed_by.as_str()),
        };
        knowledge::insert_version(&mut *tx, &version).await?;

        // Provenance: copy candidate evidence references, re-verified against
        // the project scope inside this transaction.
        for event in &evidence_events {
            let evidence = KnowledgeEvidence {
                id: EvidenceId::generate(),
                knowledge_version_id: version.id,
                event_id: Some(event.id),
                source_type: "INTERACTION_EVENT".to_string(),
                content_hash: event.content_hash.clone(),
                repository: event.repository.clone(),
                commit_sha: event.commit_sha.clone(),
                file_path: event.file_path.clone(),
                line_start: None,
                line_end: None,
                created_at: Utc::now(),
            };
            knowledge::insert_evidence(&mut *tx, &evidence).await?;
        }

        query::insert_knowledge_links(
            &mut tx,
            self.tenant_id(),
            candidate.project_id,
            version.id,
            &associated,
        )
        .await?;
        candidates::mark_promoted(&mut tx, candidate.id, version.id).await?;

        jobs::enqueue(
            &mut *tx,
            ownstate_domain::JobKind::GenerateEmbedding,
            &json!({ "knowledge_version_id": version.id }),
        )
        .await?;

        tx.commit().await?;

        tracing::info!(
            candidate_id = %candidate.id,
            knowledge_item_id = %item.id,
            knowledge_version_id = %version.id,
            version_number = version.version_number,
            trust_level = %version.trust_level,
            superseded = ?decision.supersedes_version_id.map(|id| id.to_string()),
            "candidate promoted to canonical knowledge"
        );
        Ok((item, version))
    }

    pub(crate) async fn reject_candidate(
        &self,
        id: CandidateId,
        reason: &str,
    ) -> ServiceResult<()> {
        candidates::mark_rejected(self.pool(), self.tenant_id(), id, reason).await?;
        tracing::info!(candidate_id = %id, "candidate rejected");
        Ok(())
    }

    /// Version history of one knowledge item with provenance, under the same
    /// classification ceiling as every other read path: versions above the
    /// server ceiling are omitted entirely, and the content of QUARANTINED or
    /// REVOKED versions is withheld (their identity, status and provenance
    /// remain visible — history does not disappear, but curated-out content
    /// is not re-served to models).
    pub(crate) async fn get_knowledge(
        &self,
        id: KnowledgeItemId,
    ) -> ServiceResult<KnowledgeDetail> {
        let item = knowledge::get_item(self.pool(), self.tenant_id(), id).await?;
        let ceiling = self.max_classification();
        let versions: Vec<_> = knowledge::list_versions_for_item(self.pool(), item.id)
            .await?
            .into_iter()
            .filter(|v| v.security_classification <= ceiling)
            .map(|mut v| {
                if matches!(
                    v.status,
                    ownstate_domain::KnowledgeStatus::Quarantined
                        | ownstate_domain::KnowledgeStatus::Revoked
                ) {
                    v.content = format!("[content withheld: {}]", v.status);
                    v.structured_content = None;
                }
                v
            })
            .collect();
        let version_ids: Vec<_> = versions.iter().map(|v| v.id).collect();
        let evidence = knowledge::list_evidence_for_versions(self.pool(), &version_ids).await?;
        Ok(KnowledgeDetail {
            item,
            versions,
            evidence,
        })
    }
}
