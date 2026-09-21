//! Capture assessment storage: append-only capture quality records.

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};

use ownstate_domain::enums::CaptureMode;
use ownstate_domain::*;

use crate::error::StorageError;

/// Insert a capture assessment (append-only).
#[allow(clippy::too_many_arguments)]
pub async fn insert_assessment(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: TenantId,
    project_id: ProjectId,
    session_id: Option<SessionId>,
    repository_id: Option<RepositoryId>,
    capture_mode: CaptureMode,
    channels: &serde_json::Value,
    source_adapter: &str,
    assessed_by: Option<&str>,
    notes: Option<&str>,
) -> Result<CaptureAssessmentId, StorageError> {
    let id = CaptureAssessmentId::generate();
    let now = Utc::now();

    sqlx::query(
        "INSERT INTO capture_assessments (id, tenant_id, project_id, session_id, repository_id, capture_mode, channels, source_adapter, assessed_at, assessed_by, notes)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"
    )
    .bind(id.as_uuid())
    .bind(tenant_id.as_uuid())
    .bind(project_id.as_uuid())
    .bind(session_id.map(|s| s.as_uuid()))
    .bind(repository_id.map(|r| r.as_uuid()))
    .bind(capture_mode.as_str())
    .bind(channels)
    .bind(source_adapter)
    .bind(now)
    .bind(assessed_by)
    .bind(notes)
    .execute(&mut **tx)
    .await?;

    Ok(id)
}

/// Find the most recent capture assessment for a session.
pub async fn latest_for_session(
    pool: &PgPool,
    tenant_id: TenantId,
    session_id: SessionId,
) -> Result<Option<CaptureAssessmentRow>, StorageError> {
    let row = sqlx::query_as::<_, CaptureAssessmentRow>(
        "SELECT id, tenant_id, project_id, session_id, repository_id, capture_mode, channels, source_adapter, assessed_at, assessed_by, notes
         FROM capture_assessments
         WHERE tenant_id = $1 AND session_id = $2
         ORDER BY assessed_at DESC
         LIMIT 1"
    )
    .bind(tenant_id.as_uuid())
    .bind(session_id.as_uuid())
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// Insert an idempotency key for evidence recording.
#[allow(clippy::too_many_arguments)]
pub async fn insert_idempotency_key(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: TenantId,
    project_id: ProjectId,
    source: &str,
    source_session_id: Option<&str>,
    source_event_id: &str,
    event_id: EventId,
    content_fingerprint: &str,
) -> Result<(), StorageError> {
    sqlx::query(
        "INSERT INTO recording_idempotency (id, tenant_id, project_id, source, source_session_id, source_event_id, event_id, content_fingerprint, recorded_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, clock_timestamp())
         ON CONFLICT (tenant_id, project_id, source, source_event_id) DO NOTHING"
    )
    .bind(uuid::Uuid::now_v7())
    .bind(tenant_id.as_uuid())
    .bind(project_id.as_uuid())
    .bind(source)
    .bind(source_session_id)
    .bind(source_event_id)
    .bind(event_id.as_uuid())
    .bind(content_fingerprint)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Check if an event was already recorded (idempotency).
pub async fn find_idempotency_key(
    pool: &PgPool,
    tenant_id: TenantId,
    project_id: ProjectId,
    source: &str,
    source_event_id: &str,
) -> Result<Option<uuid::Uuid>, StorageError> {
    let row = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT event_id FROM recording_idempotency
         WHERE tenant_id = $1 AND project_id = $2 AND source = $3 AND source_event_id = $4",
    )
    .bind(tenant_id.as_uuid())
    .bind(project_id.as_uuid())
    .bind(source)
    .bind(source_event_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

#[derive(Debug, sqlx::FromRow)]
pub struct CaptureAssessmentRow {
    pub id: uuid::Uuid,
    pub tenant_id: uuid::Uuid,
    pub project_id: uuid::Uuid,
    pub session_id: Option<uuid::Uuid>,
    pub repository_id: Option<uuid::Uuid>,
    pub capture_mode: String,
    pub channels: serde_json::Value,
    pub source_adapter: String,
    pub assessed_at: DateTime<Utc>,
    pub assessed_by: Option<String>,
    pub notes: Option<String>,
}
