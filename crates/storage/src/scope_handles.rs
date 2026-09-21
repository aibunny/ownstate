//! Scope handle storage: server-minted opaque references for MCP operations.

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};

use ownstate_domain::*;
use ownstate_domain::capture::ScopeHandle;
use ownstate_domain::enums::{OwnershipDomain, ResolutionState, ScopeKind};

use crate::error::StorageError;

/// Insert a new scope handle. Returns the stored handle.
#[allow(clippy::too_many_arguments)]
pub async fn insert_handle(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: TenantId,
    ownership_domain: OwnershipDomain,
    project_id: Option<ProjectId>,
    repository_id: Option<RepositoryId>,
    workspace_id: Option<WorkspaceId>,
    scope_kind: ScopeKind,
    resolution_state: ResolutionState,
    label: Option<&str>,
    expires_at: Option<DateTime<Utc>>,
) -> Result<ScopeHandle, StorageError> {
    let id = ScopeHandleId::generate();
    let now = Utc::now();

    sqlx::query(
        "INSERT INTO scope_handles (id, tenant_id, ownership_domain, project_id, repository_id, workspace_id, scope_kind, resolution_state, label, created_at, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"
    )
    .bind(id.as_uuid())
    .bind(tenant_id.as_uuid())
    .bind(ownership_domain.as_str())
    .bind(project_id.map(|p| p.as_uuid()))
    .bind(repository_id.map(|r| r.as_uuid()))
    .bind(workspace_id.map(|w| w.as_uuid()))
    .bind(scope_kind.as_str())
    .bind(resolution_state.as_str())
    .bind(label)
    .bind(now)
    .bind(expires_at)
    .execute(&mut **tx)
    .await?;

    Ok(ScopeHandle {
        id,
        tenant_id,
        ownership_domain,
        project_id,
        repository_id,
        workspace_id,
        scope_kind,
        resolution_state,
        label: label.map(String::from),
        created_at: now,
        expires_at,
        revoked_at: None,
    })
}

/// Find a scope handle by ID. Returns None if not found, revoked, or expired.
pub async fn find_handle(
    pool: &PgPool,
    tenant_id: TenantId,
    handle_id: ScopeHandleId,
    now: DateTime<Utc>,
) -> Result<Option<ScopeHandle>, StorageError> {
    let handle = sqlx::query_as::<_, ScopeHandleRow>(
        "SELECT id, tenant_id, ownership_domain, project_id, repository_id, workspace_id, scope_kind, resolution_state, label, created_at, expires_at, revoked_at
         FROM scope_handles
         WHERE id = $1 AND tenant_id = $2"
    )
    .bind(handle_id.as_uuid())
    .bind(tenant_id.as_uuid())
    .fetch_optional(pool)
    .await?;

    Ok(handle.and_then(|h| h.into_domain(now)))
}

/// Revoke a scope handle (set revoked_at).
pub async fn revoke_handle(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: TenantId,
    handle_id: ScopeHandleId,
) -> Result<(), StorageError> {
    sqlx::query(
        "UPDATE scope_handles SET revoked_at = clock_timestamp() WHERE id = $1 AND tenant_id = $2 AND revoked_at IS NULL"
    )
    .bind(handle_id.as_uuid())
    .bind(tenant_id.as_uuid())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Find existing scope handle for a project+repository combination.
pub async fn find_handle_for_scope(
    pool: &PgPool,
    tenant_id: TenantId,
    project_id: Option<ProjectId>,
    repository_id: Option<RepositoryId>,
    now: DateTime<Utc>,
) -> Result<Option<ScopeHandle>, StorageError> {
    let handle = sqlx::query_as::<_, ScopeHandleRow>(
        "SELECT id, tenant_id, ownership_domain, project_id, repository_id, workspace_id, scope_kind, resolution_state, label, created_at, expires_at, revoked_at
         FROM scope_handles
         WHERE tenant_id = $1
           AND revoked_at IS NULL
           AND (expires_at IS NULL OR expires_at > $2)
           AND project_id IS NOT DISTINCT FROM $3
           AND repository_id IS NOT DISTINCT FROM $4
         ORDER BY created_at DESC
         LIMIT 1"
    )
    .bind(tenant_id.as_uuid())
    .bind(now)
    .bind(project_id.map(|p| p.as_uuid()))
    .bind(repository_id.map(|r| r.as_uuid()))
    .fetch_optional(pool)
    .await?;

    Ok(handle.and_then(|h| h.into_domain(now)))
}

/// Internal row representation for SQLx.
#[derive(Debug, sqlx::FromRow)]
struct ScopeHandleRow {
    id: uuid::Uuid,
    tenant_id: uuid::Uuid,
    ownership_domain: String,
    project_id: Option<uuid::Uuid>,
    repository_id: Option<uuid::Uuid>,
    workspace_id: Option<uuid::Uuid>,
    scope_kind: String,
    resolution_state: String,
    label: Option<String>,
    created_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
    revoked_at: Option<DateTime<Utc>>,
}

impl ScopeHandleRow {
    fn into_domain(self, now: DateTime<Utc>) -> Option<ScopeHandle> {
        // Filter out revoked/expired handles
        if self.revoked_at.is_some()
            || self.expires_at.is_some_and(|exp| exp <= now)
        {
            return None;
        }

        let ownership_domain: OwnershipDomain = self.ownership_domain.parse().ok()?;
        let scope_kind: ScopeKind = self.scope_kind.parse().ok()?;
        let resolution_state: ResolutionState = self.resolution_state.parse().ok()?;

        Some(ScopeHandle {
            id: ScopeHandleId::from_uuid(self.id),
            tenant_id: TenantId::from_uuid(self.tenant_id),
            ownership_domain,
            project_id: self.project_id.map(ProjectId::from_uuid),
            repository_id: self.repository_id.map(RepositoryId::from_uuid),
            workspace_id: self.workspace_id.map(WorkspaceId::from_uuid),
            scope_kind,
            resolution_state,
            label: self.label,
            created_at: self.created_at,
            expires_at: self.expires_at,
            revoked_at: self.revoked_at,
        })
    }
}
