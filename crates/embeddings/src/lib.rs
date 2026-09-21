//! Embedding provider abstraction.
//!
//! Canonical knowledge is never coupled to one embedding model: providers are
//! identified by (model, model_version, dimensions) and the index keyed by
//! model, so re-embedding with a better model never rewrites knowledge.

use async_trait::async_trait;
use thiserror::Error;

mod deterministic;
#[cfg(feature = "fastembed")]
mod fastembed_provider;

pub use deterministic::DeterministicProvider;
#[cfg(feature = "fastembed")]
pub use fastembed_provider::FastembedProvider;

#[derive(Debug, Error)]
pub enum EmbeddingError {
    #[error("embedding model initialization failed: {0}")]
    Init(String),

    #[error("embedding failed: {0}")]
    Embed(String),

    #[error("provider '{0}' is not compiled into this binary")]
    ProviderUnavailable(String),
}

#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Stable identifier of the model; embedding index rows are keyed by it.
    fn model_name(&self) -> &str;

    fn model_version(&self) -> &str;

    fn dimensions(&self) -> usize;

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError>;
}

/// Which provider to instantiate, from configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Fastembed,
    Deterministic,
}

impl std::str::FromStr for ProviderKind {
    type Err = EmbeddingError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "fastembed" => Ok(ProviderKind::Fastembed),
            "deterministic" => Ok(ProviderKind::Deterministic),
            other => Err(EmbeddingError::ProviderUnavailable(other.to_string())),
        }
    }
}

/// Instantiate a provider. `cache_dir` is only used by model-downloading
/// providers.
pub fn create_provider(
    kind: ProviderKind,
    cache_dir: &std::path::Path,
) -> Result<std::sync::Arc<dyn EmbeddingProvider>, EmbeddingError> {
    match kind {
        ProviderKind::Deterministic => Ok(std::sync::Arc::new(DeterministicProvider::new())),
        #[cfg(feature = "fastembed")]
        ProviderKind::Fastembed => Ok(std::sync::Arc::new(FastembedProvider::new(cache_dir)?)),
        #[cfg(not(feature = "fastembed"))]
        ProviderKind::Fastembed => {
            let _ = cache_dir;
            Err(EmbeddingError::ProviderUnavailable("fastembed".into()))
        }
    }
}
