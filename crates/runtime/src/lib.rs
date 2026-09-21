//! Shared process bootstrap for the Ownstate binaries (api, worker, mcp):
//! configuration from environment, tracing setup, and construction of the
//! application services (pool → migrations → embedding provider).

use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use ownstate_domain::{SecurityClassification, TenantId};
use ownstate_embeddings::ProviderKind;
use ownstate_services::AppServices;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeploymentMode {
    Personal,
    Business,
}

impl FromStr for DeploymentMode {
    type Err = RuntimeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "personal" => Ok(Self::Personal),
            "business" => Ok(Self::Business),
            _ => Err(RuntimeError::Config(
                "OWNSTATE_DEPLOYMENT_MODE must be personal or business".into(),
            )),
        }
    }
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("storage error: {0}")]
    Storage(#[from] ownstate_storage::StorageError),

    #[error("embedding provider error: {0}")]
    Embedding(#[from] ownstate_embeddings::EmbeddingError),

    #[error("startup task failed: {0}")]
    Startup(String),
}

/// Default single-tenant id for personal deployments.
pub const DEFAULT_TENANT: Uuid = Uuid::from_u128(1);

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub http_addr: SocketAddr,
    /// Static bearer token for the HTTP API. `None` disables authentication
    /// (local development only; a warning is logged at startup).
    pub api_token: Option<String>,
    /// Separate curation credential. When set, promotion/rejection of
    /// candidates and HUMAN-attributed proposals require this token instead
    /// of the regular API token — a capability boundary between "can
    /// propose/read" and "can decide what becomes canonical".
    pub admin_token: Option<String>,
    pub mcp_token: Option<String>,
    pub worker_token: Option<String>,
    pub deployment_mode: DeploymentMode,
    /// Deployment-wide classification ceiling; request ceilings are clamped
    /// to it before any retrieval.
    pub max_classification: SecurityClassification,
    pub tenant_id: TenantId,
    pub embedding_provider: ProviderKind,
    pub embedding_cache_dir: PathBuf,
    pub auto_migrate: bool,
    pub worker_poll_ms: u64,
    pub max_db_connections: u32,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("database_url", &"[redacted]")
            .field("http_addr", &self.http_addr)
            .field("api_token_configured", &self.api_token.is_some())
            .field("admin_token_configured", &self.admin_token.is_some())
            .field("mcp_token_configured", &self.mcp_token.is_some())
            .field("worker_token_configured", &self.worker_token.is_some())
            .field("deployment_mode", &self.deployment_mode)
            .field("max_classification", &self.max_classification)
            .field("tenant_id", &self.tenant_id)
            .field("embedding_provider", &self.embedding_provider)
            .field("auto_migrate", &self.auto_migrate)
            .field("worker_poll_ms", &self.worker_poll_ms)
            .field("max_db_connections", &self.max_db_connections)
            .finish_non_exhaustive()
    }
}

impl Config {
    pub fn from_env() -> Result<Self, RuntimeError> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    // Pure configuration boundary: tests never mutate process-wide environment.
    fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, RuntimeError> {
        let env = |key: &str| lookup(key).filter(|v| !v.trim().is_empty());
        let invalid = |message: &str| RuntimeError::Config(message.into());
        let database_url = env("OWNSTATE_DATABASE_URL")
            .ok_or_else(|| invalid("OWNSTATE_DATABASE_URL is required"))?;
        let tenant_id = match env("OWNSTATE_TENANT_ID") {
            Some(raw) => TenantId::from_uuid(
                Uuid::from_str(&raw).map_err(|_| invalid("OWNSTATE_TENANT_ID must be a UUID"))?,
            ),
            None => TenantId::from_uuid(DEFAULT_TENANT),
        };
        let embedding_provider = match env("OWNSTATE_EMBEDDING_PROVIDER") {
            Some(raw) => ProviderKind::from_str(&raw).map_err(|_| {
                invalid("OWNSTATE_EMBEDDING_PROVIDER must be fastembed or deterministic")
            })?,
            None => ProviderKind::Fastembed,
        };
        let deployment_mode = match env("OWNSTATE_DEPLOYMENT_MODE") {
            Some(raw) => raw.parse::<DeploymentMode>()?,
            None => DeploymentMode::Personal,
        };
        let worker_poll_ms = match env("OWNSTATE_WORKER_POLL_MS") {
            Some(raw) => raw
                .parse::<u64>()
                .ok()
                .filter(|v| *v > 0)
                .ok_or_else(|| invalid("OWNSTATE_WORKER_POLL_MS must be a positive integer"))?,
            None => 1000,
        };
        let max_db_connections =
            match env("OWNSTATE_MAX_DB_CONNECTIONS") {
                Some(raw) => raw.parse::<u32>().ok().filter(|v| *v > 0).ok_or_else(|| {
                    invalid("OWNSTATE_MAX_DB_CONNECTIONS must be a positive integer")
                })?,
                None => 10,
            };
        let max_classification = match env("OWNSTATE_MAX_CLASSIFICATION") {
            Some(raw) => raw.parse::<SecurityClassification>().map_err(|_| invalid(
                "OWNSTATE_MAX_CLASSIFICATION must be PUBLIC, INTERNAL, CONFIDENTIAL, RESTRICTED or SECRET"))?,
            None => SecurityClassification::Confidential,
        };
        let http_addr = env("OWNSTATE_HTTP_ADDR")
            .unwrap_or_else(|| "127.0.0.1:8080".into())
            .parse::<SocketAddr>()
            .map_err(|_| invalid("OWNSTATE_HTTP_ADDR must be an IP socket address"))?;
        let auto_migrate = match env("OWNSTATE_AUTO_MIGRATE").as_deref() {
            None | Some("true" | "1") => true,
            Some("false" | "0") => false,
            Some(_) => return Err(invalid("OWNSTATE_AUTO_MIGRATE must be true, false, 1 or 0")),
        };
        Ok(Self {
            database_url,
            http_addr,
            api_token: env("OWNSTATE_API_TOKEN"),
            admin_token: env("OWNSTATE_ADMIN_TOKEN"),
            mcp_token: env("OWNSTATE_MCP_TOKEN"),
            worker_token: env("OWNSTATE_WORKER_TOKEN"),
            deployment_mode,
            max_classification,
            tenant_id,
            embedding_provider,
            embedding_cache_dir: env("OWNSTATE_EMBEDDING_CACHE_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(".fastembed_cache")),
            auto_migrate,
            worker_poll_ms,
            max_db_connections,
        })
    }
}

/// Initialize tracing. MCP servers speak JSON-RPC on stdout, so logs always
/// go to stderr.
pub fn init_tracing(default_filter: &str) {
    use tracing_subscriber::EnvFilter;
    use tracing_subscriber::prelude::*;

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));
    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_target(true),
        )
        .init();
}

/// Connect, migrate (when configured), build the embedding provider and the
/// application services. Model initialization can block (first-run download),
/// so it runs on the blocking pool.
pub async fn build_services(config: &Config) -> Result<Arc<AppServices>, RuntimeError> {
    let pool = ownstate_storage::connect(&config.database_url, config.max_db_connections).await?;

    if config.auto_migrate {
        ownstate_storage::run_migrations(&pool).await?;
        tracing::info!("database migrations applied");
    }

    let kind = config.embedding_provider;
    let cache_dir = config.embedding_cache_dir.clone();
    let embedder =
        tokio::task::spawn_blocking(move || ownstate_embeddings::create_provider(kind, &cache_dir))
            .await
            .map_err(|e| RuntimeError::Startup(format!("embedding init task panicked: {e}")))??;

    tracing::info!(
        model = embedder.model_name(),
        dimensions = embedder.dimensions(),
        "embedding provider ready"
    );

    Ok(Arc::new(
        AppServices::new(pool, embedder, config.tenant_id)
            .with_max_classification(config.max_classification),
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod config_tests {
    use super::*;

    fn config(values: &[(&str, &str)]) -> Result<Config, RuntimeError> {
        Config::from_lookup(|key| {
            values
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.to_string())
                .or_else(|| {
                    (key == "OWNSTATE_DATABASE_URL")
                        .then(|| "postgres://fictional:private@localhost/ownstate".into())
                })
        })
    }

    #[test]
    fn startup_rejects_invalid_typed_configuration_without_echoing_values() {
        for (key, raw) in [
            ("OWNSTATE_WORKER_POLL_MS", "0"),
            ("OWNSTATE_WORKER_POLL_MS", "-1"),
            ("OWNSTATE_MAX_DB_CONNECTIONS", "0"),
            ("OWNSTATE_MAX_DB_CONNECTIONS", "4294967296"),
            ("OWNSTATE_AUTO_MIGRATE", "tru"),
            ("OWNSTATE_HTTP_ADDR", "not-a-bind-address"),
            ("OWNSTATE_EMBEDDING_PROVIDER", "private-config-value"),
            ("OWNSTATE_MAX_CLASSIFICATION", "private-config-value"),
            ("OWNSTATE_DEPLOYMENT_MODE", "private-config-value"),
        ] {
            let error = config(&[(key, raw)]).unwrap_err().to_string();
            assert!(error.contains(key));
            assert!(!error.contains("private-config-value"));
        }
    }

    #[test]
    fn startup_defaults_overrides_and_debug_redaction() {
        let default = config(&[]).unwrap();
        assert_eq!(default.max_db_connections, 10);
        assert_eq!(default.worker_poll_ms, 1000);
        assert!(default.auto_migrate);
        let configured = config(&[
            ("OWNSTATE_MAX_DB_CONNECTIONS", "3"),
            ("OWNSTATE_WORKER_POLL_MS", "25"),
            ("OWNSTATE_AUTO_MIGRATE", "0"),
            ("OWNSTATE_HTTP_ADDR", "[::1]:8088"),
            ("OWNSTATE_API_TOKEN", "fictional-api-secret"),
            ("OWNSTATE_ADMIN_TOKEN", "fictional-admin-secret"),
            ("OWNSTATE_MCP_TOKEN", "fictional-mcp-secret"),
            ("OWNSTATE_WORKER_TOKEN", "fictional-worker-secret"),
            ("OWNSTATE_DEPLOYMENT_MODE", "business"),
        ])
        .unwrap();
        assert_eq!(configured.max_db_connections, 3);
        assert_eq!(configured.worker_poll_ms, 25);
        assert!(!configured.auto_migrate);
        assert_eq!(configured.http_addr.port(), 8088);
        assert_eq!(configured.deployment_mode, DeploymentMode::Business);
        let debug = format!("{configured:?}");
        for secret in [
            "fictional-api-secret",
            "fictional-admin-secret",
            "fictional-mcp-secret",
            "fictional-worker-secret",
            "private@localhost",
        ] {
            assert!(!debug.contains(secret));
        }
    }
}
