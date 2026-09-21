//! Embedding index rows. These are derived data over canonical knowledge:
//! upserts are allowed (re-embedding), and the canonical tables never depend
//! on them.

use ownstate_domain::{EmbeddingId, KnowledgeVersionId};
use pgvector::Vector;
use uuid::Uuid;

use crate::Result;

pub struct NewEmbedding<'a> {
    pub entity_id: KnowledgeVersionId,
    pub model: &'a str,
    pub model_version: &'a str,
    pub dimensions: i32,
    pub vector: Vector,
}

pub async fn upsert_knowledge_version_embedding<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    e: NewEmbedding<'_>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO embeddings (id, entity_type, entity_id, model, model_version, dimensions,
             embedding, created_at)
         VALUES ($1, 'KNOWLEDGE_VERSION', $2, $3, $4, $5, $6, now())
         ON CONFLICT (entity_type, entity_id, model)
         DO UPDATE SET embedding = EXCLUDED.embedding,
                       model_version = EXCLUDED.model_version,
                       dimensions = EXCLUDED.dimensions,
                       created_at = now()",
    )
    .bind(EmbeddingId::generate().as_uuid())
    .bind(e.entity_id.as_uuid())
    .bind(e.model)
    .bind(e.model_version)
    .bind(e.dimensions)
    .bind(e.vector)
    .execute(exec)
    .await?;
    Ok(())
}

pub async fn count_for_version<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    version_id: KnowledgeVersionId,
    model: &str,
) -> Result<i64> {
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM embeddings
         WHERE entity_type = 'KNOWLEDGE_VERSION' AND entity_id = $1 AND model = $2",
    )
    .bind(version_id.as_uuid())
    .bind(model)
    .fetch_one(exec)
    .await?;
    Ok(count)
}

/// Version ids among `ids` that do NOT yet have an embedding for `model`.
pub async fn missing_for_model<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    ids: &[KnowledgeVersionId],
    model: &str,
) -> Result<Vec<KnowledgeVersionId>> {
    let uuids: Vec<Uuid> = ids.iter().map(|id| id.as_uuid()).collect();
    let rows: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT v.id FROM unnest($1::uuid[]) AS v(id)
         WHERE NOT EXISTS (
             SELECT 1 FROM embeddings e
             WHERE e.entity_type = 'KNOWLEDGE_VERSION' AND e.entity_id = v.id AND e.model = $2
         )",
    )
    .bind(&uuids)
    .bind(model)
    .fetch_all(exec)
    .await?;
    Ok(rows.into_iter().map(|(id,)| id.into()).collect())
}
