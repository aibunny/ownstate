use ownstate_domain::DomainError;
use ownstate_storage::StorageError;
use thiserror::Error;

pub type ServiceResult<T> = Result<T, ServiceError>;

/// Application-level error taxonomy. Interface adapters map these onto HTTP
/// status codes / MCP errors; internal detail is logged, never surfaced.
#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("{0} not found")]
    NotFound(&'static str),

    #[error("invalid request: {0}")]
    Validation(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("forbidden: {0}")]
    Forbidden(String),

    #[error("embedding failure: {0}")]
    Embedding(String),

    #[error("internal error")]
    Internal(#[source] anyhow_like::Boxed),
}

/// Minimal boxed-error holder so we don't pull anyhow into the services API.
pub mod anyhow_like {
    #[derive(Debug)]
    pub struct Boxed(pub Box<dyn std::error::Error + Send + Sync>);

    impl std::fmt::Display for Boxed {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            self.0.fmt(f)
        }
    }

    impl std::error::Error for Boxed {}
}

impl ServiceError {
    pub fn validation(msg: impl Into<String>) -> Self {
        ServiceError::Validation(msg.into())
    }

    pub fn internal(err: impl std::error::Error + Send + Sync + 'static) -> Self {
        ServiceError::Internal(anyhow_like::Boxed(Box::new(err)))
    }

    pub fn forbidden(msg: impl Into<String>) -> Self {
        ServiceError::Forbidden(msg.into())
    }
}

impl From<StorageError> for ServiceError {
    fn from(err: StorageError) -> Self {
        match err {
            StorageError::NotFound { entity } => ServiceError::NotFound(entity),
            StorageError::Conflict(msg) => ServiceError::Conflict(msg),
            other => ServiceError::internal(other),
        }
    }
}

impl From<DomainError> for ServiceError {
    fn from(err: DomainError) -> Self {
        match err {
            DomainError::CandidateNotPending(s) => {
                ServiceError::Conflict(format!("candidate is not pending (status: {s})"))
            }
            other => ServiceError::Validation(other.to_string()),
        }
    }
}

impl From<sqlx::Error> for ServiceError {
    fn from(err: sqlx::Error) -> Self {
        ServiceError::internal(err)
    }
}
