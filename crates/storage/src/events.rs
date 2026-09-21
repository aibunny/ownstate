//! Append-only interaction event persistence. The database refuses UPDATE
//! and DELETE on this table via trigger; this module only ever inserts and
//! reads.

use ownstate_domain::{EventId, InteractionEvent, ProjectId, SessionId, TenantId};
use uuid::Uuid;

use crate::rows::EventRow;
use crate::{Result, StorageError};

const EVENT_SELECT: &str = "SELECT id, tenant_id, project_id, session_id, source, source_event_id,
        event_type, actor_type, actor_id, sequence, occurred_at, model_provider, model_name,
        content, content_hash, tool_name, tool_call_id, repository, branch, commit_sha,
        file_path, security_classification, metadata, created_at
     FROM interaction_events WHERE session_id = $1 ORDER BY sequence";

pub async fn insert<'e>(exec: impl sqlx::PgExecutor<'e>, e: &InteractionEvent) -> Result<()> {
    let result = sqlx::query(
        "INSERT INTO interaction_events (id, tenant_id, project_id, session_id, source,
             source_event_id, event_type, actor_type, actor_id, sequence, occurred_at,
             model_provider, model_name, content, content_hash, tool_name, tool_call_id,
             repository, branch, commit_sha, file_path, security_classification, metadata,
             created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17,
                 $18, $19, $20, $21, $22, $23, $24)",
    )
    .bind(e.id.as_uuid())
    .bind(e.tenant_id.as_uuid())
    .bind(e.project_id.as_uuid())
    .bind(e.session_id.as_uuid())
    .bind(&e.source)
    .bind(&e.source_event_id)
    .bind(e.event_type.as_str())
    .bind(e.actor_type.as_str())
    .bind(&e.actor_id)
    .bind(e.sequence)
    .bind(e.occurred_at)
    .bind(&e.model_provider)
    .bind(&e.model_name)
    .bind(&e.content)
    .bind(&e.content_hash)
    .bind(&e.tool_name)
    .bind(&e.tool_call_id)
    .bind(&e.repository)
    .bind(&e.branch)
    .bind(&e.commit_sha)
    .bind(&e.file_path)
    .bind(e.security_classification.as_str())
    .bind(&e.metadata)
    .bind(e.created_at)
    .execute(exec)
    .await;

    match result {
        Ok(_) => Ok(()),
        Err(err) => {
            let storage_err = StorageError::from(err);
            if storage_err.is_unique_violation() {
                Err(StorageError::Conflict(format!(
                    "sequence {} already exists in session",
                    e.sequence
                )))
            } else {
                Err(storage_err)
            }
        }
    }
}

/// Highest sequence currently stored for a session (-1 when empty). Callers
/// must hold the session lock (see `sessions::lock_for_append`) while
/// assigning subsequent sequences.
pub async fn max_sequence(tx: &mut sqlx::PgConnection, session_id: SessionId) -> Result<i64> {
    let (max,): (i64,) = sqlx::query_as(
        "SELECT COALESCE(MAX(sequence), -1) FROM interaction_events WHERE session_id = $1",
    )
    .bind(session_id.as_uuid())
    .fetch_one(tx)
    .await?;
    Ok(max)
}

pub async fn list_for_session<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    session_id: SessionId,
) -> Result<Vec<InteractionEvent>> {
    let rows: Vec<EventRow> = sqlx::query_as(EVENT_SELECT)
        .bind(session_id.as_uuid())
        .fetch_all(exec)
        .await?;
    rows.into_iter().map(TryInto::try_into).collect()
}

/// Fetch events by id, scoped to a tenant + project. Used to validate that
/// proposed evidence references really exist inside the project the candidate
/// claims to describe.
pub async fn get_scoped_by_ids<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    tenant_id: TenantId,
    project_id: ProjectId,
    ids: &[EventId],
) -> Result<Vec<InteractionEvent>> {
    let uuids: Vec<Uuid> = ids.iter().map(|id| id.as_uuid()).collect();
    let rows: Vec<EventRow> = sqlx::query_as(
        "SELECT id, tenant_id, project_id, session_id, source, source_event_id, event_type,
                actor_type, actor_id, sequence, occurred_at, model_provider, model_name,
                content, content_hash, tool_name, tool_call_id, repository, branch, commit_sha,
                file_path, security_classification, metadata, created_at
         FROM interaction_events
         WHERE id = ANY($1) AND tenant_id = $2 AND project_id = $3",
    )
    .bind(&uuids)
    .bind(tenant_id.as_uuid())
    .bind(project_id.as_uuid())
    .fetch_all(exec)
    .await?;
    rows.into_iter().map(TryInto::try_into).collect()
}
