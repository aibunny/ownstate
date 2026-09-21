use ownstate_domain::{CandidateId, CandidateKnowledge, KnowledgeVersionId, TenantId};
use uuid::Uuid;

use crate::rows::CandidateRow;
use crate::{Result, StorageError};

const CANDIDATE_SELECT: &str = "SELECT id, tenant_id, project_id, session_id, kind, subject_key,
        content, structured_content, confidence, proposed_by, source, evidence_event_ids,
        security_classification, status, rejection_reason, promoted_version_id,
        created_at, updated_at
     FROM candidate_knowledge WHERE id = $1 AND tenant_id = $2";

pub async fn insert<'e>(exec: impl sqlx::PgExecutor<'e>, c: &CandidateKnowledge) -> Result<()> {
    let evidence: Vec<Uuid> = c.evidence_event_ids.iter().map(|id| id.as_uuid()).collect();
    sqlx::query(
        "INSERT INTO candidate_knowledge (id, tenant_id, project_id, session_id, kind,
             subject_key, content, structured_content, confidence, proposed_by, source,
             evidence_event_ids, security_classification, status, rejection_reason,
             promoted_version_id, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)",
    )
    .bind(c.id.as_uuid())
    .bind(c.tenant_id.as_uuid())
    .bind(c.project_id.as_uuid())
    .bind(c.session_id.map(|s| s.as_uuid()))
    .bind(c.kind.as_str())
    .bind(&c.subject_key)
    .bind(&c.content)
    .bind(&c.structured_content)
    .bind(c.confidence)
    .bind(c.proposed_by.as_str())
    .bind(&c.source)
    .bind(&evidence)
    .bind(c.security_classification.as_str())
    .bind(c.status.as_str())
    .bind(&c.rejection_reason)
    .bind(c.promoted_version_id.map(|v| v.as_uuid()))
    .bind(c.created_at)
    .bind(c.updated_at)
    .execute(exec)
    .await?;
    Ok(())
}

pub async fn get<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    tenant_id: TenantId,
    id: CandidateId,
) -> Result<CandidateKnowledge> {
    let row: Option<CandidateRow> = sqlx::query_as(CANDIDATE_SELECT)
        .bind(id.as_uuid())
        .bind(tenant_id.as_uuid())
        .fetch_optional(exec)
        .await?;
    row.ok_or_else(|| StorageError::not_found("candidate"))?
        .try_into()
}

/// Lock the candidate row within the promotion transaction so two concurrent
/// promotions of the same candidate cannot both succeed.
pub async fn get_for_update(
    tx: &mut sqlx::PgConnection,
    tenant_id: TenantId,
    id: CandidateId,
) -> Result<CandidateKnowledge> {
    let row: Option<CandidateRow> = sqlx::query_as(
        "SELECT id, tenant_id, project_id, session_id, kind, subject_key, content,
                structured_content, confidence, proposed_by, source, evidence_event_ids,
                security_classification, status, rejection_reason, promoted_version_id,
                created_at, updated_at
         FROM candidate_knowledge WHERE id = $1 AND tenant_id = $2 FOR UPDATE",
    )
    .bind(id.as_uuid())
    .bind(tenant_id.as_uuid())
    .fetch_optional(&mut *tx)
    .await?;
    row.ok_or_else(|| StorageError::not_found("candidate"))?
        .try_into()
}

pub async fn mark_promoted(
    tx: &mut sqlx::PgConnection,
    id: CandidateId,
    version_id: KnowledgeVersionId,
) -> Result<()> {
    let result = sqlx::query(
        "UPDATE candidate_knowledge
         SET status = 'PROMOTED', promoted_version_id = $2, updated_at = now()
         WHERE id = $1 AND status = 'PENDING'",
    )
    .bind(id.as_uuid())
    .bind(version_id.as_uuid())
    .execute(tx)
    .await?;

    if result.rows_affected() == 0 {
        return Err(StorageError::Conflict(
            "candidate is no longer pending".into(),
        ));
    }
    Ok(())
}

/// How many proposals are waiting for promotion in a project. Surfaced by
/// bootstrap so an un-promoted backlog is never mistaken for an empty
/// knowledge base.
pub async fn count_pending_for_project<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    tenant_id: TenantId,
    project_id: ownstate_domain::ProjectId,
) -> Result<i64> {
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM candidate_knowledge
         WHERE tenant_id = $1 AND project_id = $2 AND status = 'PENDING'",
    )
    .bind(tenant_id.as_uuid())
    .bind(project_id.as_uuid())
    .fetch_one(exec)
    .await?;
    Ok(count)
}

pub async fn mark_rejected<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    tenant_id: TenantId,
    id: CandidateId,
    reason: &str,
) -> Result<()> {
    let result = sqlx::query(
        "UPDATE candidate_knowledge
         SET status = 'REJECTED', rejection_reason = $3, updated_at = now()
         WHERE id = $1 AND tenant_id = $2 AND status = 'PENDING'",
    )
    .bind(id.as_uuid())
    .bind(tenant_id.as_uuid())
    .bind(reason)
    .execute(exec)
    .await?;

    if result.rows_affected() == 0 {
        return Err(StorageError::Conflict(
            "candidate is no longer pending".into(),
        ));
    }
    Ok(())
}
