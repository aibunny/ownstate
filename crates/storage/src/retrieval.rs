//! Hybrid retrieval legs. Authorization inputs (tenant, project, allowed
//! classifications) are applied inside the SQL, before any row leaves the
//! database. Both legs return ranked version ids; fusion happens in the
//! services layer.

use ownstate_domain::{KnowledgeVersionId, ProjectId, TenantId};
use pgvector::Vector;
use uuid::Uuid;

use crate::Result;

/// PostgreSQL full-text leg, ranked by `ts_rank`.
#[allow(clippy::too_many_arguments)]
pub async fn fts_search<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    tenant_id: TenantId,
    project_id: ProjectId,
    query: &str,
    allowed_classifications: &[&'static str],
    kinds: Option<&[&'static str]>,
    limit: i64,
) -> Result<Vec<KnowledgeVersionId>> {
    let allowed: Vec<String> = allowed_classifications
        .iter()
        .map(|s| s.to_string())
        .collect();
    let kinds: Option<Vec<String>> = kinds.map(|ks| ks.iter().map(|s| s.to_string()).collect());

    let rows: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT kv.id
         FROM knowledge_versions_with_safe_evidence kv
         JOIN knowledge_items ki ON ki.id = kv.knowledge_item_id
         WHERE ki.tenant_id = $1
           AND ki.project_id = $2
           AND kv.status = 'ACTIVE'
           AND kv.valid_from <= statement_timestamp()
           AND (kv.valid_until IS NULL OR kv.valid_until > statement_timestamp())
           AND kv.security_classification = ANY($3)
           AND ($4::text[] IS NULL OR ki.kind = ANY($4))
           AND kv.content_tsv @@ websearch_to_tsquery('english', $5)
         ORDER BY ts_rank(kv.content_tsv, websearch_to_tsquery('english', $5)) DESC,
                  kv.id
         LIMIT $6",
    )
    .bind(tenant_id.as_uuid())
    .bind(project_id.as_uuid())
    .bind(&allowed)
    .bind(&kinds)
    .bind(query)
    .bind(limit)
    .fetch_all(exec)
    .await?;

    Ok(rows.into_iter().map(|(id,)| id.into()).collect())
}

/// pgvector cosine-similarity leg over embeddings produced by `model`.
#[allow(clippy::too_many_arguments)]
pub async fn vector_search<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    tenant_id: TenantId,
    project_id: ProjectId,
    query_embedding: Vector,
    model: &str,
    allowed_classifications: &[&'static str],
    kinds: Option<&[&'static str]>,
    limit: i64,
) -> Result<Vec<KnowledgeVersionId>> {
    let allowed: Vec<String> = allowed_classifications
        .iter()
        .map(|s| s.to_string())
        .collect();
    let kinds: Option<Vec<String>> = kinds.map(|ks| ks.iter().map(|s| s.to_string()).collect());

    let rows: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT e.entity_id
         FROM embeddings e
         JOIN knowledge_versions_with_safe_evidence kv ON kv.id = e.entity_id
         JOIN knowledge_items ki ON ki.id = kv.knowledge_item_id
         WHERE ki.tenant_id = $1
           AND ki.project_id = $2
           AND kv.status = 'ACTIVE'
           AND kv.valid_from <= statement_timestamp()
           AND (kv.valid_until IS NULL OR kv.valid_until > statement_timestamp())
           AND kv.security_classification = ANY($3)
           AND ($4::text[] IS NULL OR ki.kind = ANY($4))
           AND e.entity_type = 'KNOWLEDGE_VERSION'
           AND e.model = $5
         ORDER BY e.embedding <=> $6, e.entity_id
         LIMIT $7",
    )
    .bind(tenant_id.as_uuid())
    .bind(project_id.as_uuid())
    .bind(&allowed)
    .bind(&kinds)
    .bind(model)
    .bind(query_embedding)
    .bind(limit)
    .fetch_all(exec)
    .await?;

    Ok(rows.into_iter().map(|(id,)| id.into()).collect())
}

/// Most recently created ACTIVE versions for a project (bootstrap path — no
/// query, just "what does this project currently know").
pub async fn recent_active<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    tenant_id: TenantId,
    project_id: ProjectId,
    allowed_classifications: &[&'static str],
    limit: i64,
) -> Result<Vec<KnowledgeVersionId>> {
    let allowed: Vec<String> = allowed_classifications
        .iter()
        .map(|s| s.to_string())
        .collect();

    let rows: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT kv.id
         FROM knowledge_versions_with_safe_evidence kv
         JOIN knowledge_items ki ON ki.id = kv.knowledge_item_id
         WHERE ki.tenant_id = $1
           AND ki.project_id = $2
           AND kv.status = 'ACTIVE'
           AND kv.valid_from <= statement_timestamp()
           AND (kv.valid_until IS NULL OR kv.valid_until > statement_timestamp())
           AND kv.security_classification = ANY($3)
         ORDER BY kv.created_at DESC, kv.id
         LIMIT $4",
    )
    .bind(tenant_id.as_uuid())
    .bind(project_id.as_uuid())
    .bind(&allowed)
    .bind(limit)
    .fetch_all(exec)
    .await?;

    Ok(rows.into_iter().map(|(id,)| id.into()).collect())
}
