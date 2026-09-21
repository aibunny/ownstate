//! One typed query entry point shared by interface adapters. Plans describe
//! bounded operations; only storage's static SQL can execute them.
use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use ownstate_domain::*;
use ownstate_storage::{
    knowledge, projects,
    query::{self, QueryRead},
};
use pgvector::Vector;
use serde::Serialize;

use crate::{AppServices, ServiceError, ServiceResult};

#[derive(Debug, Serialize)]
pub struct SemanticMatch {
    pub version: KnowledgeVersion,
    pub item: KnowledgeItem,
    pub score: f64,
    pub evidence: Vec<KnowledgeEvidence>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QuerySnapshot {
    Structured {
        result: StructuredResult,
    },
    Semantic {
        matches: Vec<SemanticMatch>,
    },
    Relationship {
        versions: Vec<InstitutionalVersion>,
    },
    Hybrid {
        matched_entity_ids: Vec<EntityId>,
        matched_entity_count: i64,
        has_more_entities: bool,
        matches: Vec<SemanticMatch>,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QueryResult {
    Snapshot {
        as_of: DateTime<Utc>,
        /// Aggregate totals are exact; entity, relationship, and semantic rows
        /// obey the requested response limit.
        bounded_by_limit: bool,
        /// Semantic ranking is approximate: at most 4 * limit IDs per leg,
        /// fused and trust-weighted before response truncation.
        semantic_candidate_limit: Option<u32>,
        snapshot: QuerySnapshot,
    },
    Temporal {
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        bounded_by_limit: bool,
        semantic_candidate_limit: Option<u32>,
        before: QuerySnapshot,
        after: QuerySnapshot,
        /// Snapshot set changes are not a claim that intermediate transitions
        /// never happened. Recorded changes are returned separately.
        added_version_ids: Vec<uuid::Uuid>,
        removed_version_ids: Vec<uuid::Uuid>,
        recorded_changes: Box<TemporalChanges>,
        recorded_change_evidence: Vec<KnowledgeEvidence>,
    },
}

fn snapshot_ids(snapshot: &QuerySnapshot) -> HashSet<uuid::Uuid> {
    match snapshot {
        QuerySnapshot::Structured {
            result: StructuredResult::Entities { entities },
        } => entities.iter().map(|e| e.version.id.as_uuid()).collect(),
        QuerySnapshot::Semantic { matches } | QuerySnapshot::Hybrid { matches, .. } => {
            matches.iter().map(|h| h.version.id.as_uuid()).collect()
        }
        QuerySnapshot::Relationship { versions } => {
            versions.iter().map(|v| v.id.as_uuid()).collect()
        }
        _ => HashSet::new(),
    }
}

impl AppServices {
    pub(crate) async fn execute_query(&self, plan: QueryPlan) -> ServiceResult<QueryResult> {
        plan.validate().map_err(ServiceError::validation)?;
        let constraints = plan.constraints();
        // Admission happens before lexical, vector, or structured retrieval.
        projects::get(self.pool(), self.tenant_id(), constraints.project_id).await?;
        let ceiling = self.clamp_classification(Some(constraints.max_classification));
        let allowed = SecurityClassification::allowed_up_to(ceiling);
        let q = QueryRead {
            tenant: self.tenant_id(),
            constraints,
            allowed: &allowed,
            as_of: match constraints.as_of {
                Some(at) => at,
                None => query::snapshot_timestamp(self.pool()).await?,
            },
        };
        if let QueryPlan::Temporal {
            start,
            end,
            subject,
            ..
        } = &plan
        {
            let before = self
                .query_temporal_snapshot(QueryRead { as_of: *start, ..q }, subject)
                .await?;
            let after = self
                .query_temporal_snapshot(QueryRead { as_of: *end, ..q }, subject)
                .await?;
            let semantic_embedding = match subject {
                TemporalSubject::Semantic { query: text } => match self
                    .embedder()
                    .embed(&[text.trim().to_string()])
                    .await
                {
                    Ok(mut vectors) if !vectors.is_empty() => Some(query::TemporalSemanticInput {
                        embedding: Vector::from(vectors.remove(0)),
                        model: self.embedder().model_name(),
                    }),
                    Ok(_) => None,
                    Err(_) => {
                        tracing::warn!("temporal query embedding failed; lexical history retained");
                        None
                    }
                },
                TemporalSubject::Structured { .. } => None,
            };
            let recorded_changes = query::temporal_changes_with_semantic(
                self.pool(),
                q,
                *start,
                *end,
                subject,
                semantic_embedding,
            )
            .await?;
            let changed_knowledge_ids: Vec<_> = recorded_changes
                .knowledge
                .iter()
                .map(|(v, _)| v.id)
                .collect();
            let recorded_change_evidence =
                knowledge::list_evidence_for_versions(self.pool(), &changed_knowledge_ids).await?;
            let before_ids = snapshot_ids(&before);
            let after_ids = snapshot_ids(&after);
            let mut added_version_ids: Vec<_> =
                after_ids.difference(&before_ids).copied().collect();
            let mut removed_version_ids: Vec<_> =
                before_ids.difference(&after_ids).copied().collect();
            added_version_ids.sort();
            removed_version_ids.sort();
            return Ok(QueryResult::Temporal {
                start: *start,
                end: *end,
                semantic_candidate_limit: matches!(subject, TemporalSubject::Semantic { .. })
                    .then_some(constraints.limit * 8),
                bounded_by_limit: !matches!(
                    subject,
                    TemporalSubject::Structured {
                        operation: StructuredOperation::Count
                            | StructuredOperation::GroupByJurisdiction
                    }
                ),
                before,
                after,
                added_version_ids,
                removed_version_ids,
                recorded_changes: Box::new(recorded_changes),
                recorded_change_evidence,
            });
        }
        let (snapshot, bounded_by_limit) = match &plan {
            QueryPlan::Structured { operation, .. } => (
                QuerySnapshot::Structured {
                    result: query::structured(self.pool(), q, *operation).await?,
                },
                matches!(operation, StructuredOperation::Entities),
            ),
            QueryPlan::Semantic { query: text, .. } => (
                QuerySnapshot::Semantic {
                    matches: self.query_semantic(q, text, false).await?,
                },
                true,
            ),
            QueryPlan::Relationship {
                entity_id,
                relationship_type,
                direction,
                ..
            } => (
                QuerySnapshot::Relationship {
                    versions: query::relationships(
                        self.pool(),
                        q,
                        *entity_id,
                        *direction,
                        relationship_type.as_deref(),
                    )
                    .await?,
                },
                true,
            ),
            QueryPlan::Hybrid { query: text, .. } => {
                let entities = query::hybrid_entities(self.pool(), q).await?;
                let matched_entity_count =
                    match query::structured(self.pool(), q, StructuredOperation::Count).await? {
                        StructuredResult::Count { count, .. } => count,
                        _ => return Err(ServiceError::validation("invalid hybrid count response")),
                    };
                let matches = self.query_semantic(q, text, true).await?;
                (
                    QuerySnapshot::Hybrid {
                        has_more_entities: matched_entity_count > entities.len() as i64,
                        matched_entity_ids: entities,
                        matched_entity_count,
                        matches,
                    },
                    true,
                )
            }
            QueryPlan::Temporal { .. } => {
                return Err(ServiceError::validation("invalid temporal query"));
            }
        };
        Ok(QueryResult::Snapshot {
            semantic_candidate_limit: matches!(
                &plan,
                QueryPlan::Semantic { .. } | QueryPlan::Hybrid { .. }
            )
            .then_some(constraints.limit * 8),
            as_of: q.as_of,
            bounded_by_limit,
            snapshot,
        })
    }

    async fn query_temporal_snapshot(
        &self,
        q: QueryRead<'_>,
        subject: &TemporalSubject,
    ) -> ServiceResult<QuerySnapshot> {
        match subject {
            TemporalSubject::Structured { operation } => Ok(QuerySnapshot::Structured {
                result: query::structured(self.pool(), q, *operation).await?,
            }),
            TemporalSubject::Semantic { query: text } => Ok(QuerySnapshot::Semantic {
                matches: self.query_semantic(q, text, false).await?,
            }),
        }
    }

    async fn query_semantic(
        &self,
        q: QueryRead<'_>,
        text: &str,
        hybrid: bool,
    ) -> ServiceResult<Vec<SemanticMatch>> {
        let lexical = if hybrid {
            query::hybrid_fts(self.pool(), q, text).await?
        } else {
            query::semantic_fts(self.pool(), q, text, None).await?
        };
        let dense = match self.embedder().embed(&[text.trim().to_string()]).await {
            Ok(mut vectors) if !vectors.is_empty() => {
                let embedding = Vector::from(vectors.remove(0));
                if hybrid {
                    query::hybrid_vector(self.pool(), q, embedding, self.embedder().model_name())
                        .await?
                } else {
                    query::semantic_vector(
                        self.pool(),
                        q,
                        embedding,
                        self.embedder().model_name(),
                        None,
                    )
                    .await?
                }
            }
            Ok(_) => Vec::new(),
            Err(_) => {
                tracing::warn!("typed query embedding failed; lexical results retained");
                Vec::new()
            }
        };
        let mut scores: HashMap<KnowledgeVersionId, f64> = HashMap::new();
        for leg in [lexical, dense] {
            for (rank, id) in leg.into_iter().enumerate() {
                *scores.entry(id).or_default() += 1.0 / (60.0 + rank as f64 + 1.0);
            }
        }
        let ids: Vec<_> = scores.keys().copied().collect();
        // A ranked ID is not an authorization grant. Final load repeats scope,
        // validity, provenance and every typed association constraint.
        let loaded = if hybrid {
            query::load_hybrid_ranked(self.pool(), q, &ids).await?
        } else {
            query::load_ranked(self.pool(), q, &ids).await?
        };
        let mut hits: Vec<_> = loaded
            .into_iter()
            .map(|(version, item)| {
                let score = scores.get(&version.id).copied().unwrap_or_default()
                    * version.trust_level.rank_weight();
                SemanticMatch {
                    version,
                    item,
                    score,
                    evidence: Vec::new(),
                }
            })
            .collect();
        hits.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.version.id.cmp(&b.version.id))
        });
        hits.truncate(q.constraints.limit as usize);
        let ids: Vec<_> = hits.iter().map(|h| h.version.id).collect();
        let mut evidence: HashMap<KnowledgeVersionId, Vec<KnowledgeEvidence>> = HashMap::new();
        for e in knowledge::list_evidence_for_versions(self.pool(), &ids).await? {
            evidence.entry(e.knowledge_version_id).or_default().push(e);
        }
        for hit in &mut hits {
            hit.evidence = evidence.remove(&hit.version.id).unwrap_or_default();
        }
        Ok(hits)
    }
}
