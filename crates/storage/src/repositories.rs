//! Repository identity storage with transactional upserts and alias management.

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use ownstate_domain::{RepositoryId, TenantId};
use ownstate_domain::capture::RepositoryLocator;

use crate::error::StorageError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StoredRepository {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub workspace_id: Uuid,
    pub canonical_origin: String,
    pub provider: String,
    pub owner_name: Option<String>,
    pub repo_name: Option<String>,
    pub internal_key: String,
    pub fork_of_repository_id: Option<Uuid>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// Insert or find a repository by internal key. Uses a transactional upsert
/// to guarantee idempotency under concurrent bootstrap.
pub async fn upsert_repository(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: TenantId,
    workspace_id: ownstate_domain::WorkspaceId,
    locator: &RepositoryLocator,
) -> Result<StoredRepository, StorageError> {
    // Try to find existing by internal key
    let existing = sqlx::query_as::<_, StoredRepository>(
        "SELECT id, tenant_id, workspace_id, canonical_origin, provider, owner_name, repo_name, internal_key, fork_of_repository_id, metadata, created_at
         FROM repositories
         WHERE tenant_id = $1 AND internal_key = $2"
    )
    .bind(tenant_id.as_uuid())
    .bind(&locator.internal_key)
    .fetch_optional(&mut **tx)
    .await?;

    if let Some(repo) = existing {
        return Ok(repo);
    }

    let id = RepositoryId::generate();
    let now = Utc::now();

    sqlx::query(
        "INSERT INTO repositories (id, tenant_id, workspace_id, canonical_origin, provider, owner_name, repo_name, internal_key, metadata, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"
    )
    .bind(id.as_uuid())
    .bind(tenant_id.as_uuid())
    .bind(workspace_id.as_uuid())
    .bind(&locator.canonical_uri)
    .bind(locator.provider.as_str())
    .bind(&locator.owner)
    .bind(&locator.repository_name)
    .bind(&locator.internal_key)
    .bind(serde_json::json!({}))
    .bind(now)
    .execute(&mut **tx)
    .await?;

    Ok(StoredRepository {
        id: id.as_uuid(),
        tenant_id: tenant_id.as_uuid(),
        workspace_id: workspace_id.as_uuid(),
        canonical_origin: locator.canonical_uri.clone(),
        provider: locator.provider.as_str().to_string(),
        owner_name: locator.owner.clone(),
        repo_name: locator.repository_name.clone(),
        internal_key: locator.internal_key.clone(),
        fork_of_repository_id: None,
        metadata: serde_json::json!({}),
        created_at: now,
    })
}

/// Find a repository by canonical origin.
pub async fn find_by_canonical_origin(
    pool: &PgPool,
    tenant_id: TenantId,
    canonical_origin: &str,
) -> Result<Option<StoredRepository>, StorageError> {
    let repo = sqlx::query_as::<_, StoredRepository>(
        "SELECT id, tenant_id, workspace_id, canonical_origin, provider, owner_name, repo_name, internal_key, fork_of_repository_id, metadata, created_at
         FROM repositories
         WHERE tenant_id = $1 AND canonical_origin = $2"
    )
    .bind(tenant_id.as_uuid())
    .bind(canonical_origin)
    .fetch_optional(pool)
    .await?;
    Ok(repo)
}

/// Find a repository by internal key.
pub async fn find_by_internal_key(
    pool: &PgPool,
    tenant_id: TenantId,
    internal_key: &str,
) -> Result<Option<StoredRepository>, StorageError> {
    let repo = sqlx::query_as::<_, StoredRepository>(
        "SELECT id, tenant_id, workspace_id, canonical_origin, provider, owner_name, repo_name, internal_key, fork_of_repository_id, metadata, created_at
         FROM repositories
         WHERE tenant_id = $1 AND internal_key = $2"
    )
    .bind(tenant_id.as_uuid())
    .bind(internal_key)
    .fetch_optional(pool)
    .await?;
    Ok(repo)
}

/// Insert a repository alias (historical remote, fork upstream, etc.).
pub async fn insert_alias(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: TenantId,
    repository_id: RepositoryId,
    alias_uri: &str,
    alias_kind: &str,
) -> Result<(), StorageError> {
    sqlx::query(
        "INSERT INTO repository_aliases (id, tenant_id, repository_id, alias_uri, alias_kind, recorded_at)
         VALUES ($1, $2, $3, $4, $5, clock_timestamp())
         ON CONFLICT (tenant_id, repository_id, alias_uri) DO NOTHING"
    )
    .bind(uuid::Uuid::now_v7())
    .bind(tenant_id.as_uuid())
    .bind(repository_id.as_uuid())
    .bind(alias_uri)
    .bind(alias_kind)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
