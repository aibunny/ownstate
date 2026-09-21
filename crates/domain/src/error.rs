use thiserror::Error;

/// Domain-level failures. Infrastructure layers wrap these; they never leak
/// database or transport details upward.
#[derive(Debug, Error)]
pub enum DomainError {
    #[error("invalid {type} value: {value}")]
    InvalidEnumValue { r#type: &'static str, value: String },

    #[error("validation failed: {0}")]
    Validation(String),

    #[error("candidate is not pending (status: {0})")]
    CandidateNotPending(String),

    #[error("invalid remote URL: {reason}")]
    InvalidRemoteUrl { reason: String },
}

impl DomainError {
    pub fn validation(msg: impl Into<String>) -> Self {
        DomainError::Validation(msg.into())
    }
}
