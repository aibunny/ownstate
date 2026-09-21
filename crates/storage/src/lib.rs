//! PostgreSQL persistence for Ownstate.
//!
//! PostgreSQL is the canonical system of record. This crate owns every SQL
//! statement in the codebase; all queries are static, parameterized strings.
//! Domain invariants that PostgreSQL can enforce (append-only evidence,
//! immutable knowledge versions, one ACTIVE version per item) live in the
//! migrations as constraints and triggers — not in convention.

pub mod candidates;
pub mod capture;
pub mod context_packets;
pub mod embeddings;
pub mod error;
pub mod events;
pub mod institutional;
pub mod jobs;
pub mod knowledge;
pub mod policy;
pub mod projects;
pub mod query;
pub mod repositories;
pub mod retrieval;
pub mod rows;
pub mod scope_handles;
pub mod sessions;

#[cfg(feature = "test-support")]
pub mod test_support;

pub use error::StorageError;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

pub type Result<T> = std::result::Result<T, StorageError>;

/// Embedded migrations from the workspace `migrations/` directory.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

pub async fn connect(database_url: &str, max_connections: u32) -> Result<PgPool> {
    Ok(PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(database_url)
        .await?)
}

pub async fn run_migrations(pool: &PgPool) -> Result<()> {
    MIGRATOR.run(pool).await.map_err(StorageError::Migration)?;
    Ok(())
}
