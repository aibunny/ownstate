//! Hybrid project-scoped retrieval: PostgreSQL full-text + pgvector cosine
//! similarity, fused with Reciprocal Rank Fusion and weighted by trust.
//!
//! Authorization (tenant, project, classification ceiling) is applied inside
//! the SQL of both legs — results are scoped before they leave the database.

use std::collections::HashMap;

use ownstate_domain::{
    KnowledgeEvidence, KnowledgeItem, KnowledgeKind, KnowledgeVersion, KnowledgeVersionId,
    ProjectId, SecurityClassification, limits,
};
use ownstate_storage::{knowledge, projects, retrieval};
use pgvector::Vector;

use crate::{AppServices, ServiceError, ServiceResult};

const RRF_K: f64 = 60.0;
/// Overfetch factor per leg so fusion has enough candidates to reorder.
const LEG_FETCH_MULTIPLIER: i64 = 3;

pub struct SearchParams {
    pub project_id: ProjectId,
    pub query: String,
    pub kinds: Option<Vec<KnowledgeKind>>,
    pub limit: usize,
    pub max_classification: SecurityClassification,
}

pub struct SearchHit {
    pub version: KnowledgeVersion,
    pub item: KnowledgeItem,
    pub score: f64,
    pub evidence: Vec<KnowledgeEvidence>,
}

impl AppServices {
    pub(crate) async fn search_knowledge(&self, p: SearchParams) -> ServiceResult<Vec<SearchHit>> {
        let query = p.query.trim();
        if query.is_empty() || query.len() > 1024 {
            return Err(ServiceError::validation(
                "query must be 1..=1024 characters",
            ));
        }
        if p.limit == 0 || p.limit > limits::MAX_CONTEXT_ITEMS {
            return Err(ServiceError::validation(format!(
                "limit must be 1..={}",
                limits::MAX_CONTEXT_ITEMS
            )));
        }
        // Authorization before retrieval.
        let project = projects::get(self.pool(), self.tenant_id(), p.project_id).await?;

        // The requested ceiling can only narrow the server-side maximum.
        let ceiling = self.clamp_classification(Some(p.max_classification));
        let allowed = SecurityClassification::allowed_up_to(ceiling);
        let kind_strs: Option<Vec<&'static str>> = p
            .kinds
            .as_ref()
            .map(|ks| ks.iter().map(|k| k.as_str()).collect());
        let leg_limit = (p.limit as i64) * LEG_FETCH_MULTIPLIER;

        let fts_ranked = retrieval::fts_search(
            self.pool(),
            self.tenant_id(),
            project.id,
            query,
            &allowed,
            kind_strs.as_deref(),
            leg_limit,
        )
        .await?;

        // The vector leg degrades gracefully: an embedding failure loses
        // semantic recall but keeps lexical retrieval working.
        let vec_ranked = match self.embedder().embed(&[query.to_string()]).await {
            Ok(mut vectors) if !vectors.is_empty() => {
                let query_vec = Vector::from(vectors.remove(0));
                retrieval::vector_search(
                    self.pool(),
                    self.tenant_id(),
                    project.id,
                    query_vec,
                    self.embedder().model_name(),
                    &allowed,
                    kind_strs.as_deref(),
                    leg_limit,
                )
                .await?
            }
            Ok(_) => Vec::new(),
            Err(err) => {
                tracing::warn!(error = %err, "query embedding failed; vector leg skipped");
                Vec::new()
            }
        };

        let fused = fuse_rrf(&[fts_ranked, vec_ranked]);
        if fused.is_empty() {
            return Ok(Vec::new());
        }

        let ids: Vec<KnowledgeVersionId> = fused.iter().map(|(id, _)| *id).collect();
        let loaded = knowledge::load_retrievable_versions_with_items(
            self.pool(),
            self.tenant_id(),
            project.id,
            &ids,
            &allowed,
        )
        .await?;
        let mut by_id: HashMap<KnowledgeVersionId, (KnowledgeVersion, KnowledgeItem)> =
            loaded.into_iter().map(|(v, i)| (v.id, (v, i))).collect();

        let mut hits: Vec<SearchHit> = fused
            .into_iter()
            .filter_map(|(id, rrf_score)| {
                by_id.remove(&id).map(|(version, item)| {
                    let score = rrf_score * version.trust_level.rank_weight();
                    SearchHit {
                        version,
                        item,
                        score,
                        evidence: Vec::new(),
                    }
                })
            })
            .collect();

        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.version.id.cmp(&b.version.id))
        });
        hits.truncate(p.limit);

        // Attach provenance to what survived ranking.
        let hit_ids: Vec<KnowledgeVersionId> = hits.iter().map(|h| h.version.id).collect();
        let mut evidence_by_version: HashMap<KnowledgeVersionId, Vec<KnowledgeEvidence>> =
            HashMap::new();
        for e in knowledge::list_evidence_for_versions(self.pool(), &hit_ids).await? {
            evidence_by_version
                .entry(e.knowledge_version_id)
                .or_default()
                .push(e);
        }
        for hit in &mut hits {
            if let Some(ev) = evidence_by_version.remove(&hit.version.id) {
                hit.evidence = ev;
            }
        }

        tracing::debug!(
            project_id = %project.id,
            results = hits.len(),
            "hybrid search completed"
        );
        Ok(hits)
    }
}

/// Reciprocal Rank Fusion over ranked id lists. Stable and parameter-light;
/// exactly what the spec asks for before anything cleverer is justified.
fn fuse_rrf(legs: &[Vec<KnowledgeVersionId>]) -> Vec<(KnowledgeVersionId, f64)> {
    let mut scores: HashMap<KnowledgeVersionId, f64> = HashMap::new();
    for leg in legs {
        for (rank, id) in leg.iter().enumerate() {
            *scores.entry(*id).or_insert(0.0) += 1.0 / (RRF_K + rank as f64 + 1.0);
        }
    }
    let mut fused: Vec<_> = scores.into_iter().collect();
    fused.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    fused
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn rrf_prefers_ids_present_in_both_legs() {
        let a = KnowledgeVersionId::generate();
        let b = KnowledgeVersionId::generate();
        let c = KnowledgeVersionId::generate();
        let fused = fuse_rrf(&[vec![a, b], vec![b, c]]);
        assert_eq!(fused[0].0, b);
    }
}
