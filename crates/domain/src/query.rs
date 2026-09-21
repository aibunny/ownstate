//! Validated query descriptions, never executable SQL or model authority.
use crate::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryConstraints {
    pub project_id: ProjectId,
    #[serde(default)]
    pub as_of: Option<DateTime<Utc>>,
    pub max_classification: SecurityClassification,
    #[serde(default)]
    pub entity_kind: Option<EntityKind>,
    #[serde(default)]
    pub knowledge_kinds: Vec<KnowledgeKind>,
    #[serde(default)]
    pub entity_ids: Vec<EntityId>,
    #[serde(default)]
    pub relationship: Option<RelationshipConstraint>,
    #[serde(default)]
    pub trust_levels: Vec<TrustLevel>,
    #[serde(default)]
    pub source_types: Vec<String>,
    pub limit: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationshipConstraint {
    pub relationship_type: String,
    #[serde(default)]
    pub target_entity_ids: Vec<EntityId>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StructuredOperation {
    Entities,
    Count,
    GroupByJurisdiction,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RelationshipDirection {
    Outgoing,
    Incoming,
    Both,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum TemporalSubject {
    Structured { operation: StructuredOperation },
    Semantic { query: String },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum QueryPlan {
    Structured {
        constraints: QueryConstraints,
        operation: StructuredOperation,
    },
    Semantic {
        constraints: QueryConstraints,
        query: String,
    },
    Relationship {
        constraints: QueryConstraints,
        entity_id: EntityId,
        #[serde(default)]
        relationship_type: Option<String>,
        direction: RelationshipDirection,
    },
    Temporal {
        constraints: QueryConstraints,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        subject: TemporalSubject,
    },
    Hybrid {
        constraints: QueryConstraints,
        query: String,
    },
}
fn label(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 200
        && text == text.trim()
        && text
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ' ' | ':'))
}
impl QueryConstraints {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=100).contains(&self.limit)
            || self.entity_ids.len() > 100
            || self.knowledge_kinds.len() > 32
            || self.trust_levels.len() > 16
            || self.source_types.len() > 32
            || self.source_types.iter().any(|s| !label(s))
        {
            return Err("query constraints exceed bounds or contain invalid source labels".into());
        }
        if let Some(r) = &self.relationship
            && (!label(&r.relationship_type) || r.target_entity_ids.len() > 100)
        {
            return Err("invalid relationship constraint".into());
        }
        Ok(())
    }
}
impl QueryPlan {
    pub fn constraints(&self) -> &QueryConstraints {
        match self {
            Self::Structured { constraints, .. }
            | Self::Semantic { constraints, .. }
            | Self::Relationship { constraints, .. }
            | Self::Temporal { constraints, .. }
            | Self::Hybrid { constraints, .. } => constraints,
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        self.constraints().validate()?;
        let query = match self {
            Self::Semantic { query, .. } | Self::Hybrid { query, .. } => Some(query),
            Self::Temporal {
                start,
                end,
                subject,
                ..
            } => {
                if start >= end || (*end - *start) > chrono::Duration::days(366) {
                    return Err("temporal range must be positive and at most 366 days".into());
                }
                match subject {
                    TemporalSubject::Semantic { query } => Some(query),
                    _ => None,
                }
            }
            Self::Relationship {
                relationship_type: Some(t),
                ..
            } if !label(t) => return Err("invalid relationship type".into()),
            _ => None,
        };
        if query.is_some_and(|q| q.trim().is_empty() || q.len() > 4096) {
            return Err("semantic query must be 1..=4096 bytes".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StructuredResult {
    Entities {
        entities: Vec<Entity>,
    },
    Count {
        count: i64,
        evidence_event_ids: Vec<EventId>,
    },
    GroupByJurisdiction {
        groups: Vec<JurisdictionCount>,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JurisdictionCount {
    pub jurisdiction: Option<Entity>,
    pub count: i64,
    pub evidence_event_ids: Vec<EventId>,
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    fn plan() -> serde_json::Value {
        serde_json::json!({"type":"STRUCTURED","constraints":{"project_id":ProjectId::generate(),"max_classification":"INTERNAL","limit":10},"operation":"COUNT"})
    }
    #[test]
    fn rejects_sql_authority_unknown_fields_and_unbounded_limits() {
        let mut p = plan();
        p["sql"] = serde_json::json!("SELECT * FROM interaction_events");
        assert!(serde_json::from_value::<QueryPlan>(p).is_err());
        let mut p = plan();
        p["constraints"]["limit"] = serde_json::json!(101);
        assert!(
            serde_json::from_value::<QueryPlan>(p)
                .unwrap()
                .validate()
                .is_err()
        );
        let mut p = plan();
        p["constraints"]["relationship"] = serde_json::json!({"relationship_type":"x'; DROP TABLE projects;--","target_entity_ids":[]});
        assert!(
            serde_json::from_value::<QueryPlan>(p)
                .unwrap()
                .validate()
                .is_err()
        );
    }
    #[test]
    fn structured_plan_needs_no_semantic_query() {
        assert!(
            serde_json::from_value::<QueryPlan>(plan())
                .unwrap()
                .validate()
                .is_ok()
        );
    }
}

/// Interval changelog complements endpoint snapshots, retaining transient
/// assertions and semantic versions. Totals are computed before limiting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalChanges {
    pub institutional: Vec<InstitutionalVersion>,
    pub knowledge: Vec<(KnowledgeVersion, KnowledgeItem)>,
    pub total: i64,
    pub has_more: bool,
    /// Semantic interval totals cover at most this many candidates per leg.
    /// None means the total covers the full structured interval population.
    pub candidate_limit: Option<u32>,
}
