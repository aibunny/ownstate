//! Application services: the single implementation of Ownstate's use cases.
//!
//! Every interface adapter (HTTP API, MCP server, worker) calls into this
//! crate; none of them re-implement retrieval, promotion or authorization
//! logic. Tenant scope is fixed at construction (single-tenant personal mode)
//! and applied to every query before data is read.

pub mod bootstrap;
pub mod context;
pub mod error;
pub mod institutional;
pub mod jobs;
pub mod knowledge;
pub mod policy;
pub mod projects;
pub mod query;
pub mod search;
pub mod sessions;

use std::sync::Arc;

use ownstate_domain::{SecurityClassification, TenantId};
use ownstate_embeddings::EmbeddingProvider;
use sqlx::PgPool;

pub use error::{ServiceError, ServiceResult};

pub struct AppServices {
    pool: PgPool,
    embedder: Arc<dyn EmbeddingProvider>,
    tenant_id: TenantId,
    /// Server-side classification ceiling. Requests may ask for a LOWER
    /// ceiling, never a higher one — callers (and the models behind them)
    /// do not decide what they are authorized to receive.
    max_classification: SecurityClassification,
}

impl AppServices {
    pub fn new(pool: PgPool, embedder: Arc<dyn EmbeddingProvider>, tenant_id: TenantId) -> Self {
        Self {
            pool,
            embedder,
            tenant_id,
            max_classification: SecurityClassification::Confidential,
        }
    }

    /// Override the deployment-wide classification ceiling (configuration,
    /// not request input).
    pub fn with_max_classification(mut self, ceiling: SecurityClassification) -> Self {
        self.max_classification = ceiling;
        self
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub fn tenant_id(&self) -> TenantId {
        self.tenant_id
    }

    pub fn max_classification(&self) -> SecurityClassification {
        self.max_classification
    }

    /// Clamp a caller-requested ceiling to the server-side maximum.
    pub(crate) fn clamp_classification(
        &self,
        requested: Option<SecurityClassification>,
    ) -> SecurityClassification {
        requested
            .unwrap_or(self.max_classification)
            .min(self.max_classification)
    }

    pub(crate) fn embedder(&self) -> &Arc<dyn EmbeddingProvider> {
        &self.embedder
    }

    /// Cheap connectivity probe for readiness checks.
    pub async fn ping(&self) -> ServiceResult<()> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }
}
