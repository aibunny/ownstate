//! Repository identity storage with transactional upserts and alias management.

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use ownstate_domain::capture::RepositoryLocator;
use ownstate_domain::{RepositoryId, TenantId};

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

/// Insert or find a repository by internal key. The single INSERT/ON CONFLICT
/// statement is the concurrency boundary: concurrent bootstrap requests for
/// the same canonical repository return the same row.
pub async fn upsert_repository(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: TenantId,
    workspace_id: ownstate_domain::WorkspaceId,
    locator: &RepositoryLocator,
) -> Result<StoredRepository, StorageError> {
    let id = RepositoryId::generate();
    let now = Utc::now();
    let repository = sqlx::query_as::<_, StoredRepository>(
        "INSERT INTO repositories (id, tenant_id, workspace_id, canonical_origin, provider, owner_name, repo_name, internal_key, metadata, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         ON CONFLICT (tenant_id, internal_key) WHERE internal_key <> ''
         DO UPDATE SET internal_key = EXCLUDED.internal_key
         WHERE repositories.workspace_id = EXCLUDED.workspace_id
           AND repositories.canonical_origin = EXCLUDED.canonical_origin
         RETURNING id, tenant_id, workspace_id, canonical_origin, provider,
                   owner_name, repo_name, internal_key, fork_of_repository_id,
                   metadata, created_at",
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
    .fetch_optional(&mut **tx)
    .await?;

    repository.ok_or_else(|| {
        StorageError::Conflict(
            "repository identity is already bound to a different workspace".to_string(),
        )
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
