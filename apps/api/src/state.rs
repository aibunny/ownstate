use std::sync::Arc;

use ownstate_services::AppServices;
use ownstate_services::ServiceResult;
use ownstate_services::policy::AuthenticatedPrincipal;

#[derive(Clone)]
pub struct AppState {
    pub services: Arc<AppServices>,
    /// Explicit Personal-mode fallback. It exists only when neither HTTP
    /// credential is configured; Business mode never receives one.
    pub personal_fallback: Option<AuthenticatedPrincipal>,
    /// Dedicated, least-privilege principal used only by the authenticated
    /// Streamable HTTP MCP endpoint. `None` keeps `/mcp` disabled.
    pub mcp_principal: Option<AuthenticatedPrincipal>,
    /// Exact public authorities accepted by the MCP transport's DNS-rebinding
    /// protection. Loopback authorities are always included for local tests.
    pub mcp_allowed_hosts: Vec<String>,
}

impl AppState {
    pub async fn new(
        services: Arc<AppServices>,
        api_token: Option<String>,
        admin_token: Option<String>,
        mcp_token: Option<String>,
        mcp_allowed_hosts: Vec<String>,
    ) -> ServiceResult<Self> {
        let hash = |t: &String| ownstate_domain::content_hash(t.as_bytes());
        let api_hash = api_token.as_ref().map(hash);
        let admin_hash = admin_token.as_ref().map(hash);
        let mcp_hash = mcp_token.as_ref().map(hash);
        let actors = services
            .bootstrap_personal_policy_actors(
                api_hash.as_deref(),
                admin_hash.as_deref(),
                mcp_hash.as_deref(),
            )
            .await?;
        Ok(Self {
            services,
            personal_fallback: (api_token.is_none() && admin_token.is_none())
                .then_some(actors.owner),
            mcp_principal: mcp_token.is_some().then_some(actors.mcp),
            mcp_allowed_hosts,
        })
    }

    pub fn business(
        services: Arc<AppServices>,
        mcp_principal: Option<AuthenticatedPrincipal>,
        mcp_allowed_hosts: Vec<String>,
    ) -> Self {
        Self {
            services,
            personal_fallback: None,
            mcp_principal,
            mcp_allowed_hosts,
        }
    }
}
