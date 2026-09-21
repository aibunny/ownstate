use ownstate_domain::DomainError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("{entity} not found")]
    NotFound { entity: &'static str },

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("stored data failed domain validation: {0}")]
    InvalidData(#[from] DomainError),
}

impl StorageError {
    pub fn not_found(entity: &'static str) -> Self {
        StorageError::NotFound { entity }
    }

    /// True when the underlying database error is a unique-constraint
    /// violation (used to map into user-facing conflicts).
    pub fn is_unique_violation(&self) -> bool {
        match self {
            StorageError::Sqlx(sqlx::Error::Database(db)) => db.is_unique_violation(),
            _ => false,
        }
    }
}
