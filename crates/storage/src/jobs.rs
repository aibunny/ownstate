//! PostgreSQL-backed durable job queue. Claims use FOR UPDATE SKIP LOCKED so
//! multiple workers cooperate without a broker.

use chrono::{DateTime, Utc};
use ownstate_domain::{Job, JobId, JobKind};
use serde_json::Value as Json;

use crate::rows::JobRow;
use crate::{Result, StorageError};

pub async fn enqueue<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    kind: JobKind,
    payload: &Json,
) -> Result<JobId> {
    let id = JobId::generate();
    sqlx::query(
        "INSERT INTO jobs (id, kind, payload, status, run_at, created_at, updated_at)
         VALUES ($1, $2, $3, 'PENDING', now(), now(), now())",
    )
    .bind(id.as_uuid())
    .bind(kind.as_str())
    .bind(payload)
    .execute(exec)
    .await?;
    Ok(id)
}

/// Claim the next runnable job. Returns `None` when the queue is empty.
pub async fn claim_next<'e>(exec: impl sqlx::PgExecutor<'e>) -> Result<Option<Job>> {
    let row: Option<JobRow> = sqlx::query_as(
        "UPDATE jobs
         SET status = 'RUNNING', attempts = attempts + 1, claimed_at = now(), updated_at = now()
         WHERE id = (
             SELECT id FROM jobs
             WHERE status = 'PENDING' AND run_at <= now()
             ORDER BY run_at, id
             LIMIT 1
             FOR UPDATE SKIP LOCKED
         )
         RETURNING id, kind, payload, status, attempts, max_attempts, run_at, claimed_at,
                   finished_at, last_error, created_at, updated_at",
    )
    .fetch_optional(exec)
    .await?;
    row.map(TryInto::try_into).transpose()
}

pub async fn complete<'e>(exec: impl sqlx::PgExecutor<'e>, id: JobId) -> Result<()> {
    let result = sqlx::query(
        "UPDATE jobs SET status = 'SUCCEEDED', finished_at = now(), updated_at = now()
         WHERE id = $1 AND status = 'RUNNING'",
    )
    .bind(id.as_uuid())
    .execute(exec)
    .await?;
    if result.rows_affected() == 0 {
        return Err(StorageError::Conflict("job is not running".into()));
    }
    Ok(())
}

/// Record a failure. Retries with the caller-computed `retry_at` until
/// attempts exhaust `max_attempts`, then parks the job as FAILED.
pub async fn fail<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    id: JobId,
    error: &str,
    retry_at: DateTime<Utc>,
) -> Result<()> {
    let result = sqlx::query(
        "UPDATE jobs SET
             status = CASE WHEN attempts >= max_attempts THEN 'FAILED' ELSE 'PENDING' END,
             finished_at = CASE WHEN attempts >= max_attempts THEN now() ELSE NULL END,
             run_at = CASE WHEN attempts >= max_attempts THEN run_at ELSE $3 END,
             last_error = $2,
             updated_at = now()
         WHERE id = $1 AND status = 'RUNNING'",
    )
    .bind(id.as_uuid())
    .bind(error)
    .bind(retry_at)
    .execute(exec)
    .await?;
    if result.rows_affected() == 0 {
        return Err(StorageError::Conflict("job is not running".into()));
    }
    Ok(())
}

/// Requeue jobs stuck RUNNING longer than `stale_after` — a worker that
/// crashed or was shut down mid-job leaves its claim behind; without this the
/// job would be stranded forever.
pub async fn requeue_stale<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    stale_after: chrono::Duration,
) -> Result<u64> {
    let cutoff = Utc::now() - stale_after;
    let result = sqlx::query(
        "UPDATE jobs SET status = 'PENDING', updated_at = now()
         WHERE status = 'RUNNING' AND claimed_at < $1",
    )
    .bind(cutoff)
    .execute(exec)
    .await?;
    Ok(result.rows_affected())
}

pub async fn count_pending<'e>(exec: impl sqlx::PgExecutor<'e>, kind: JobKind) -> Result<i64> {
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM jobs WHERE kind = $1 AND status = 'PENDING'")
            .bind(kind.as_str())
            .fetch_one(exec)
            .await?;
    Ok(count)
}
