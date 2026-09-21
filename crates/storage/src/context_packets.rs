use ownstate_domain::{ContextPacket, ContextPacketId, TenantId};
use uuid::Uuid;

use crate::rows::ContextPacketRow;
use crate::{Result, StorageError};

pub async fn insert<'e>(exec: impl sqlx::PgExecutor<'e>, p: &ContextPacket) -> Result<()> {
    let version_ids: Vec<Uuid> = p
        .knowledge_version_ids
        .iter()
        .map(|id| id.as_uuid())
        .collect();
    sqlx::query(
        "INSERT INTO context_packets (id, tenant_id, project_id, session_id, provider, model,
             agent, task, max_items, token_budget, max_classification, knowledge_version_ids,
             estimated_tokens, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)",
    )
    .bind(p.id.as_uuid())
    .bind(p.tenant_id.as_uuid())
    .bind(p.project_id.as_uuid())
    .bind(p.session_id.map(|s| s.as_uuid()))
    .bind(&p.provider)
    .bind(&p.model)
    .bind(&p.agent)
    .bind(&p.task)
    .bind(p.max_items)
    .bind(p.token_budget)
    .bind(p.max_classification.as_str())
    .bind(&version_ids)
    .bind(p.estimated_tokens)
    .bind(p.created_at)
    .execute(exec)
    .await?;
    Ok(())
}

pub async fn get<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    tenant_id: TenantId,
    id: ContextPacketId,
) -> Result<ContextPacket> {
    let row: Option<ContextPacketRow> = sqlx::query_as(
        "SELECT id, tenant_id, project_id, session_id, provider, model, agent, task,
                max_items, token_budget, max_classification, knowledge_version_ids,
                estimated_tokens, created_at
         FROM context_packets WHERE id = $1 AND tenant_id = $2",
    )
    .bind(id.as_uuid())
    .bind(tenant_id.as_uuid())
    .fetch_optional(exec)
    .await?;
    row.ok_or_else(|| StorageError::not_found("context packet"))?
        .try_into()
}
