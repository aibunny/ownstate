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
}

impl AppState {
    pub async fn new(
        services: Arc<AppServices>,
        api_token: Option<String>,
        admin_token: Option<String>,
    ) -> ServiceResult<Self> {
        let hash = |t: &String| ownstate_domain::content_hash(t.as_bytes());
        let api_hash = api_token.as_ref().map(hash);
        let admin_hash = admin_token.as_ref().map(hash);
        let actors = services
            .bootstrap_personal_policy_actors(api_hash.as_deref(), admin_hash.as_deref())
            .await?;
        Ok(Self {
            services,
            personal_fallback: (api_token.is_none() && admin_token.is_none())
                .then_some(actors.owner),
        })
    }

    pub fn business(services: Arc<AppServices>) -> Self {
        Self {
            services,
            personal_fallback: None,
        }
    }
}
