//! Canonical knowledge persistence: items, immutable versions, provenance.

use chrono::{DateTime, Utc};
use ownstate_domain::{
    KnowledgeEvidence, KnowledgeItem, KnowledgeItemId, KnowledgeKind, KnowledgeVersion,
    KnowledgeVersionId, ProjectId, TenantId,
};
use uuid::Uuid;

use crate::rows::{EvidenceRow, KnowledgeItemRow, KnowledgeVersionRow};
use crate::{Result, StorageError};

pub async fn insert_item<'e>(exec: impl sqlx::PgExecutor<'e>, item: &KnowledgeItem) -> Result<()> {
    sqlx::query(
        "INSERT INTO knowledge_items (id, tenant_id, project_id, ownership_domain, kind,
             subject_key, created_at, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(item.id.as_uuid())
    .bind(item.tenant_id.as_uuid())
    .bind(item.project_id.as_uuid())
    .bind(item.ownership_domain.as_str())
    .bind(item.kind.as_str())
    .bind(&item.subject_key)
    .bind(item.created_at)
    .bind(&item.created_by)
    .execute(exec)
    .await?;
    Ok(())
}

pub async fn get_item<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    tenant_id: TenantId,
    id: KnowledgeItemId,
) -> Result<KnowledgeItem> {
    let row: Option<KnowledgeItemRow> = sqlx::query_as(
        "SELECT id, tenant_id, project_id, ownership_domain, kind, subject_key, created_at,
                created_by
         FROM knowledge_items WHERE id = $1 AND tenant_id = $2",
    )
    .bind(id.as_uuid())
    .bind(tenant_id.as_uuid())
    .fetch_optional(exec)
    .await?;
    row.ok_or_else(|| StorageError::not_found("knowledge item"))?
        .try_into()
}

/// Find the item identity for a (project, kind, subject) triple, locking it
/// for the surrounding promotion transaction when it exists.
pub async fn find_item_for_update(
    tx: &mut sqlx::PgConnection,
    tenant_id: TenantId,
    project_id: ProjectId,
    kind: KnowledgeKind,
    subject_key: &str,
) -> Result<Option<KnowledgeItem>> {
    let row: Option<KnowledgeItemRow> = sqlx::query_as(
        "SELECT id, tenant_id, project_id, ownership_domain, kind, subject_key, created_at,
                created_by
         FROM knowledge_items
         WHERE tenant_id = $1 AND project_id = $2 AND kind = $3 AND subject_key = $4
         FOR UPDATE",
    )
    .bind(tenant_id.as_uuid())
    .bind(project_id.as_uuid())
    .bind(kind.as_str())
    .bind(subject_key)
    .fetch_optional(&mut *tx)
    .await?;
    row.map(TryInto::try_into).transpose()
}

pub async fn get_active_version(
    tx: &mut sqlx::PgConnection,
    item_id: KnowledgeItemId,
) -> Result<Option<KnowledgeVersion>> {
    let row: Option<KnowledgeVersionRow> = sqlx::query_as(
        "SELECT id, knowledge_item_id, version_number, content, structured_content, status,
                trust_level, confidence, security_classification, valid_from, valid_until,
                supersedes_version_id, candidate_id, created_at, created_by
         FROM knowledge_versions
         WHERE knowledge_item_id = $1 AND status = 'ACTIVE'",
    )
    .bind(item_id.as_uuid())
    .fetch_optional(&mut *tx)
    .await?;
    row.map(TryInto::try_into).transpose()
}

pub async fn max_version_number(
    tx: &mut sqlx::PgConnection,
    item_id: KnowledgeItemId,
) -> Result<i32> {
    let (max,): (i32,) = sqlx::query_as(
        "SELECT COALESCE(MAX(version_number), 0) FROM knowledge_versions
         WHERE knowledge_item_id = $1",
    )
    .bind(item_id.as_uuid())
    .fetch_one(tx)
    .await?;
    Ok(max)
}

pub async fn insert_version<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    v: &KnowledgeVersion,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO knowledge_versions (id, knowledge_item_id, version_number, content,
             structured_content, status, trust_level, confidence, security_classification,
             valid_from, valid_until, supersedes_version_id, candidate_id, created_at,
             created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)",
    )
    .bind(v.id.as_uuid())
    .bind(v.knowledge_item_id.as_uuid())
    .bind(v.version_number)
    .bind(&v.content)
    .bind(&v.structured_content)
    .bind(v.status.as_str())
    .bind(v.trust_level.as_str())
    .bind(v.confidence)
    .bind(v.security_classification.as_str())
    .bind(v.valid_from)
    .bind(v.valid_until)
    .bind(v.supersedes_version_id.map(|id| id.as_uuid()))
    .bind(v.candidate_id.map(|id| id.as_uuid()))
    .bind(v.created_at)
    .bind(&v.created_by)
    .execute(exec)
    .await?;
    Ok(())
}

/// Choose one PostgreSQL timestamp after acquiring the item lock. Application
/// and database clocks can differ; validity queries use the database clock.
pub async fn promotion_timestamp(tx: &mut sqlx::PgConnection) -> Result<DateTime<Utc>> {
    let (timestamp,): (DateTime<Utc>,) = sqlx::query_as("SELECT clock_timestamp()")
        .fetch_one(tx)
        .await?;
    Ok(timestamp)
}

/// Transition an ACTIVE version to SUPERSEDED, closing its validity window.
/// Content is untouched — the database trigger would refuse anything else.
pub async fn supersede_version(
    tx: &mut sqlx::PgConnection,
    version_id: KnowledgeVersionId,
) -> Result<()> {
    let timestamp = promotion_timestamp(tx).await?;
    supersede_version_at(tx, version_id, timestamp).await
}

/// Use the same boundary for the previous valid_until and new valid_from.
pub async fn supersede_version_at(
    tx: &mut sqlx::PgConnection,
    version_id: KnowledgeVersionId,
    timestamp: DateTime<Utc>,
) -> Result<()> {
    let result = sqlx::query(
        "UPDATE knowledge_versions
         SET status = 'SUPERSEDED', valid_until = $2
         WHERE id = $1 AND status = 'ACTIVE' AND valid_from <= $2",
    )
    .bind(version_id.as_uuid())
    .bind(timestamp)
    .execute(tx)
    .await?;

    if result.rows_affected() == 0 {
        return Err(StorageError::Conflict(
            "version to supersede is not ACTIVE at the requested boundary".into(),
        ));
    }
    Ok(())
}

/// Lifecycle transition used by curation (revoke, quarantine, mark stale).
pub async fn set_version_status<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    version_id: KnowledgeVersionId,
    from: &'static str,
    to: &'static str,
) -> Result<()> {
    let result = sqlx::query(
        "UPDATE knowledge_versions SET status = $3, valid_until = now()
         WHERE id = $1 AND status = $2",
    )
    .bind(version_id.as_uuid())
    .bind(from)
    .bind(to)
    .execute(exec)
    .await?;

    if result.rows_affected() == 0 {
        return Err(StorageError::Conflict(format!(
            "version is not in status {from}"
        )));
    }
    Ok(())
}

/// Internal (worker) lookup of a version by id, without tenant scoping.
/// Only reachable from trusted job-processing code paths.
pub async fn get_version_unscoped<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    id: KnowledgeVersionId,
) -> Result<KnowledgeVersion> {
    let row: Option<KnowledgeVersionRow> = sqlx::query_as(
        "SELECT id, knowledge_item_id, version_number, content, structured_content, status,
                trust_level, confidence, security_classification, valid_from, valid_until,
                supersedes_version_id, candidate_id, created_at, created_by
         FROM knowledge_versions WHERE id = $1",
    )
    .bind(id.as_uuid())
    .fetch_optional(exec)
    .await?;
    row.ok_or_else(|| StorageError::not_found("knowledge version"))?
        .try_into()
}

pub async fn list_versions_for_item<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    item_id: KnowledgeItemId,
) -> Result<Vec<KnowledgeVersion>> {
    let rows: Vec<KnowledgeVersionRow> = sqlx::query_as(
        "SELECT id, knowledge_item_id, version_number, content, structured_content, status,
                trust_level, confidence, security_classification, valid_from, valid_until,
                supersedes_version_id, candidate_id, created_at, created_by
         FROM knowledge_versions_with_safe_evidence
         WHERE knowledge_item_id = $1
         ORDER BY version_number",
    )
    .bind(item_id.as_uuid())
    .fetch_all(exec)
    .await?;
    rows.into_iter().map(TryInto::try_into).collect()
}

/// Revalidate ranked IDs at the point content leaves PostgreSQL. Earlier
/// retrieval legs are only candidate selection: lifecycle may have changed
/// since those statements. Order is NOT preserved; callers re-rank.
pub async fn load_retrievable_versions_with_items<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    tenant_id: TenantId,
    project_id: ProjectId,
    version_ids: &[KnowledgeVersionId],
    allowed_classifications: &[&'static str],
) -> Result<Vec<(KnowledgeVersion, KnowledgeItem)>> {
    let uuids: Vec<Uuid> = version_ids.iter().map(|id| id.as_uuid()).collect();
    let rows: Vec<crate::rows::VersionItemJoinRow> = sqlx::query_as(
        "SELECT kv.id, kv.knowledge_item_id, kv.version_number, kv.content,
                kv.structured_content, kv.status, kv.trust_level, kv.confidence,
                kv.security_classification, kv.valid_from, kv.valid_until,
                kv.supersedes_version_id, kv.candidate_id, kv.created_at, kv.created_by,
                ki.id AS item_id, ki.tenant_id AS item_tenant_id,
                ki.project_id AS item_project_id,
                ki.ownership_domain AS item_ownership_domain, ki.kind AS item_kind,
                ki.subject_key AS item_subject_key, ki.created_at AS item_created_at,
                ki.created_by AS item_created_by
         FROM knowledge_versions_with_safe_evidence kv
         JOIN knowledge_items ki ON ki.id = kv.knowledge_item_id
         WHERE kv.id = ANY($1) AND ki.tenant_id = $2 AND ki.project_id = $3
           AND kv.security_classification = ANY($4)
           AND kv.status = 'ACTIVE'
           AND kv.valid_from <= statement_timestamp()
           AND (kv.valid_until IS NULL OR kv.valid_until > statement_timestamp())",
    )
    .bind(&uuids)
    .bind(tenant_id.as_uuid())
    .bind(project_id.as_uuid())
    .bind(allowed_classifications)
    .fetch_all(exec)
    .await?;

    rows.into_iter().map(TryInto::try_into).collect()
}

pub async fn insert_evidence<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    e: &KnowledgeEvidence,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO knowledge_evidence (id, knowledge_version_id, event_id, source_type,
             content_hash, repository, commit_sha, file_path, line_start, line_end, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
    )
    .bind(e.id.as_uuid())
    .bind(e.knowledge_version_id.as_uuid())
    .bind(e.event_id.map(|id| id.as_uuid()))
    .bind(&e.source_type)
    .bind(&e.content_hash)
    .bind(&e.repository)
    .bind(&e.commit_sha)
    .bind(&e.file_path)
    .bind(e.line_start)
    .bind(e.line_end)
    .bind(e.created_at)
    .execute(exec)
    .await?;
    Ok(())
}

pub async fn list_evidence_for_versions<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    version_ids: &[KnowledgeVersionId],
) -> Result<Vec<KnowledgeEvidence>> {
    let uuids: Vec<Uuid> = version_ids.iter().map(|id| id.as_uuid()).collect();
    let rows: Vec<EvidenceRow> = sqlx::query_as(
        "SELECT id, knowledge_version_id, event_id, source_type, content_hash, repository,
                commit_sha, file_path, line_start, line_end, created_at
         FROM knowledge_evidence
         WHERE knowledge_version_id = ANY($1)
         ORDER BY created_at",
    )
    .bind(&uuids)
    .fetch_all(exec)
    .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}
