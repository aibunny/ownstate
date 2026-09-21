use ownstate_domain::{Session, SessionId, TenantId};

use crate::rows::SessionRow;
use crate::{Result, StorageError};

pub async fn insert<'e>(exec: impl sqlx::PgExecutor<'e>, s: &Session) -> Result<()> {
    sqlx::query(
        "INSERT INTO sessions (id, tenant_id, project_id, source, source_session_id, agent,
             provider, model, status, started_at, ended_at, repository, initial_commit,
             final_commit, metadata, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)",
    )
    .bind(s.id.as_uuid())
    .bind(s.tenant_id.as_uuid())
    .bind(s.project_id.as_uuid())
    .bind(&s.source)
    .bind(&s.source_session_id)
    .bind(&s.agent)
    .bind(&s.provider)
    .bind(&s.model)
    .bind(s.status.as_str())
    .bind(s.started_at)
    .bind(s.ended_at)
    .bind(&s.repository)
    .bind(&s.initial_commit)
    .bind(&s.final_commit)
    .bind(&s.metadata)
    .bind(s.created_at)
    .execute(exec)
    .await?;
    Ok(())
}

pub async fn get<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    tenant_id: TenantId,
    id: SessionId,
) -> Result<Session> {
    let row: Option<SessionRow> = sqlx::query_as(
        "SELECT id, tenant_id, project_id, source, source_session_id, agent, provider, model,
                status, started_at, ended_at, repository, initial_commit, final_commit,
                metadata, created_at
         FROM sessions WHERE id = $1 AND tenant_id = $2",
    )
    .bind(id.as_uuid())
    .bind(tenant_id.as_uuid())
    .fetch_optional(exec)
    .await?;

    row.ok_or_else(|| StorageError::not_found("session"))?
        .try_into()
}

/// Lock the session row for the duration of the surrounding transaction.
/// Serializes sequence assignment for concurrent event appends.
pub async fn lock_for_append(
    tx: &mut sqlx::PgConnection,
    tenant_id: TenantId,
    id: SessionId,
) -> Result<Session> {
    let row: Option<SessionRow> = sqlx::query_as(
        "SELECT id, tenant_id, project_id, source, source_session_id, agent, provider, model,
                status, started_at, ended_at, repository, initial_commit, final_commit,
                metadata, created_at
         FROM sessions WHERE id = $1 AND tenant_id = $2 FOR UPDATE",
    )
    .bind(id.as_uuid())
    .bind(tenant_id.as_uuid())
    .fetch_optional(&mut *tx)
    .await?;

    row.ok_or_else(|| StorageError::not_found("session"))?
        .try_into()
}
