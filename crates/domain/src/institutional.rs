//! Typed institutional proposals and temporal canonical state. No model may
//! supply canonical status, trust, tenant scope, or recording timestamps.
use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EntityKind {
    Person,
    Organization,
    LegalEntity,
    Jurisdiction,
    Customer,
    Investor,
    Partner,
    Vendor,
    Project,
    Product,
    Repository,
    Contract,
    Artifact,
    Deck,
    Fundraise,
    Requirement,
    Risk,
    Issue,
    Decision,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum GraphProposal {
    Entity {
        kind: EntityKind,
        name: String,
        #[serde(default)]
        external_ids: BTreeMap<String, String>,
        #[serde(default)]
        aliases: Vec<String>,
    },
    Relationship {
        source_entity_id: EntityId,
        target_entity_id: EntityId,
        relationship_type: String,
    },
    Claim {
        subject_entity_id: EntityId,
        predicate: String,
        object: Value,
    },
}

impl GraphProposal {
    pub fn validate(&self) -> Result<(), String> {
        fn text(value: &str) -> bool {
            !value.trim().is_empty() && value.len() <= 200 && value == value.trim()
        }
        match self {
            Self::Entity {
                name,
                external_ids,
                aliases,
                ..
            } => {
                if !text(name)
                    || aliases.len() > 64
                    || external_ids.len() > 32
                    || aliases.iter().any(|a| !text(a))
                    || external_ids.iter().any(|(k, v)| !text(k) || !text(v))
                {
                    return Err("invalid entity name, aliases, or external identifiers".into());
                }
            }
            Self::Relationship {
                relationship_type, ..
            } if !text(relationship_type) => {
                return Err("relationship_type must be 1..=200 bytes and trimmed".into());
            }
            Self::Claim {
                predicate, object, ..
            } if !text(predicate) || object.is_null() || object.to_string().len() > 65_536 => {
                return Err("invalid claim predicate or object".into());
            }
            _ => {}
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphCandidate {
    pub id: GraphCandidateId,
    pub tenant_id: TenantId,
    pub project_id: ProjectId,
    pub proposal: GraphProposal,
    pub evidence_event_ids: Vec<EventId>,
    pub security_classification: SecurityClassification,
    pub proposed_by: ProposerKind,
    pub confidence: Option<f32>,
    pub status: CandidateStatus,
    pub observed_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// Immutable content and provenance with independently recorded and valid time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstitutionalVersion {
    pub id: AssertionVersionId,
    pub entity_id: Option<EntityId>,
    pub assertion_id: Option<AssertionId>,
    pub tenant_id: TenantId,
    pub project_id: ProjectId,
    pub version_number: i32,
    pub proposal: GraphProposal,
    pub valid_from: DateTime<Utc>,
    pub valid_until: Option<DateTime<Utc>>,
    pub observed_at: DateTime<Utc>,
    pub recorded_at: DateTime<Utc>,
    pub status: KnowledgeStatus,
    pub trust_level: TrustLevel,
    pub confidence: Option<f32>,
    pub security_classification: SecurityClassification,
    pub evidence_event_ids: Vec<EventId>,
    pub candidate_id: GraphCandidateId,
    pub supersedes_version_id: Option<AssertionVersionId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: EntityId,
    pub version: InstitutionalVersion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityResolution {
    pub matches: Vec<Entity>,
    /// Ambiguous aliases are returned as alternatives, never silently merged.
    pub ambiguous: bool,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn proposal_cannot_claim_canonical_authority_or_lifecycle() {
        let mut value = serde_json::json!({"type":"ENTITY","kind":"LEGAL_ENTITY","name":"Fictional Atlas","aliases":[],"external_ids":{}});
        assert!(serde_json::from_value::<GraphProposal>(value.clone()).is_ok());
        value["trust_level"] = serde_json::json!("HUMAN_EXPLICIT");
        assert!(serde_json::from_value::<GraphProposal>(value.clone()).is_err());
        value.as_object_mut().unwrap().remove("trust_level");
        value["status"] = serde_json::json!("ACTIVE");
        assert!(serde_json::from_value::<GraphProposal>(value).is_err());
    }
    #[test]
    fn empty_atomic_claim_is_not_a_valid_assertion() {
        let proposal = GraphProposal::Claim {
            subject_entity_id: EntityId::generate(),
            predicate: "registered_in".into(),
            object: Value::Null,
        };
        assert!(proposal.validate().is_err());
    }
}
