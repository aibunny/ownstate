use ownstate_domain::{Project, ProjectId, TenantId};
use uuid::Uuid;

use crate::rows::ProjectRow;
use crate::{Result, StorageError};

pub async fn insert<'e>(exec: impl sqlx::PgExecutor<'e>, project: &Project) -> Result<()> {
    sqlx::query(
        "INSERT INTO projects (id, tenant_id, ownership_domain, name, description, metadata, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(project.id.as_uuid())
    .bind(project.tenant_id.as_uuid())
    .bind(project.ownership_domain.as_str())
    .bind(&project.name)
    .bind(&project.description)
    .bind(&project.metadata)
    .bind(project.created_at)
    .execute(exec)
    .await?;
    Ok(())
}

pub async fn get<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    tenant_id: TenantId,
    id: ProjectId,
) -> Result<Project> {
    let row: Option<ProjectRow> = sqlx::query_as(
        "SELECT id, tenant_id, ownership_domain, name, description, metadata, created_at
         FROM projects WHERE id = $1 AND tenant_id = $2",
    )
    .bind(id.as_uuid())
    .bind(tenant_id.as_uuid())
    .fetch_optional(exec)
    .await?;

    row.ok_or_else(|| StorageError::not_found("project"))?
        .try_into()
}

/// Look a project up by its (tenant-unique, case-insensitive) name.
pub async fn find_by_name<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    tenant_id: TenantId,
    name: &str,
) -> Result<Option<Project>> {
    let row: Option<crate::rows::ProjectRow> = sqlx::query_as(
        "SELECT id, tenant_id, ownership_domain, name, description, metadata, created_at
         FROM projects WHERE tenant_id = $1 AND lower(name) = lower($2)",
    )
    .bind(tenant_id.as_uuid())
    .bind(name)
    .fetch_optional(exec)
    .await?;
    row.map(TryInto::try_into).transpose()
}

/// Identity lookup by recorded git origin — the strongest workspace identity:
/// the same repository is the same project wherever it is cloned and however
/// the directory is named. Oldest row wins if metadata was ever duplicated.
pub async fn find_by_git_origin<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    tenant_id: TenantId,
    origin: &str,
) -> Result<Option<Project>> {
    let row: Option<crate::rows::ProjectRow> = sqlx::query_as(
        "SELECT id, tenant_id, ownership_domain, name, description, metadata, created_at
         FROM projects
         WHERE tenant_id = $1 AND metadata->>'git_origin' = $2
         ORDER BY created_at
         LIMIT 1",
    )
    .bind(tenant_id.as_uuid())
    .bind(origin)
    .fetch_optional(exec)
    .await?;
    row.map(TryInto::try_into).transpose()
}

/// Identity lookup by the workspace's recorded filesystem location.
pub async fn find_by_root_path<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    tenant_id: TenantId,
    root_path: &str,
) -> Result<Option<Project>> {
    let row: Option<crate::rows::ProjectRow> = sqlx::query_as(
        "SELECT id, tenant_id, ownership_domain, name, description, metadata, created_at
         FROM projects
         WHERE tenant_id = $1 AND metadata->>'root_path' = $2
         ORDER BY created_at
         LIMIT 1",
    )
    .bind(tenant_id.as_uuid())
    .bind(root_path)
    .fetch_optional(exec)
    .await?;
    row.map(TryInto::try_into).transpose()
}

/// Replace a project's metadata document (projects are mutable app records,
/// unlike evidence and knowledge versions).
pub async fn update_metadata<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    tenant_id: TenantId,
    id: ProjectId,
    metadata: &serde_json::Value,
) -> Result<()> {
    let result = sqlx::query("UPDATE projects SET metadata = $3 WHERE id = $1 AND tenant_id = $2")
        .bind(id.as_uuid())
        .bind(tenant_id.as_uuid())
        .bind(metadata)
        .execute(exec)
        .await?;
    if result.rows_affected() == 0 {
        return Err(StorageError::not_found("project"));
    }
    Ok(())
}

/// Aggregate counts for a project (bootstrap summaries).
pub async fn counts<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    tenant_id: TenantId,
    id: ProjectId,
) -> Result<(i64, i64, i64)> {
    let row: (i64, i64, i64) = sqlx::query_as(
        "SELECT
            (SELECT COUNT(*) FROM sessions s
              WHERE s.project_id = $1 AND s.tenant_id = $2),
            (SELECT COUNT(*) FROM interaction_events e
              WHERE e.project_id = $1 AND e.tenant_id = $2),
            (SELECT COUNT(*) FROM knowledge_versions kv
              JOIN knowledge_items ki ON ki.id = kv.knowledge_item_id
              WHERE ki.project_id = $1 AND ki.tenant_id = $2 AND kv.status = 'ACTIVE')",
    )
    .bind(id.as_uuid())
    .bind(tenant_id.as_uuid())
    .fetch_one(exec)
    .await?;
    Ok(row)
}

/// Resolve the tenant a project belongs to without loading the whole row.
pub async fn tenant_of<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    id: ProjectId,
) -> Result<Option<TenantId>> {
    let tenant: Option<(Uuid,)> = sqlx::query_as("SELECT tenant_id FROM projects WHERE id = $1")
        .bind(id.as_uuid())
        .fetch_optional(exec)
        .await?;
    Ok(tenant.map(|(t,)| t.into()))
}
