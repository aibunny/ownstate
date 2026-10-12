use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::middleware;
use axum::routing::{get, post};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::{StreamableHttpServerConfig, StreamableHttpService};
use tower_http::trace::TraceLayer;

use crate::auth::{require_bearer, require_capture_bearer, require_mcp_bearer};
use crate::handlers;
use crate::oauth;
use crate::site;
use crate::state::AppState;

/// 4 MiB request cap: event batches are bounded server-side anyway, and large
/// content belongs in object storage (future milestone).
const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;

pub fn build(state: AppState) -> Router {
    let protected = Router::new()
        .route("/projects", post(handlers::create_project))
        .route("/projects/{id}", get(handlers::get_project))
        .route("/sessions", post(handlers::create_session))
        .route("/sessions/{id}", get(handlers::get_session))
        .route(
            "/sessions/{id}/events",
            post(handlers::append_events).get(handlers::list_events),
        )
        .route("/knowledge/proposals", post(handlers::propose_knowledge))
        .route(
            "/knowledge/proposals/{id}/promote",
            post(handlers::promote_candidate),
        )
        .route(
            "/knowledge/proposals/{id}/reject",
            post(handlers::reject_candidate),
        )
        .route("/knowledge/search", get(handlers::search_knowledge))
        .route("/query", post(handlers::execute_query))
        .route("/knowledge/{id}", get(handlers::get_knowledge))
        .route("/context/compile", post(handlers::compile_context))
        .route(
            "/institutional/proposals",
            post(crate::institutional::propose),
        )
        .route(
            "/institutional/proposals/{id}/promote",
            post(crate::institutional::promote),
        )
        .route("/entities", get(crate::institutional::entities))
        .route("/entities/resolve", get(crate::institutional::resolve))
        .route("/entities/{id}", get(crate::institutional::get_entity))
        .route(
            "/entities/{id}/relationships",
            get(crate::institutional::entity_relationships),
        )
        .route(
            "/institutional/{id}/history",
            get(crate::institutional::history),
        )
        .route("/relationships", get(crate::institutional::relationships))
        .route("/claims", get(crate::institutional::claims))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            require_bearer,
        ));

    let mut app = Router::new()
        .route("/health", get(handlers::health))
        .route("/ready", get(handlers::ready))
        .merge(protected);

    if state.account_mcp_enabled {
        app = app.merge(
            Router::new()
                .route(
                    "/capture/conversations",
                    post(crate::conversations::capture).get(crate::conversations::list),
                )
                .route(
                    "/capture/conversations/{id}/events",
                    get(crate::conversations::events),
                )
                .layer(middleware::from_fn_with_state(
                    state.clone(),
                    require_capture_bearer,
                )),
        );
        app = app.merge(
            Router::new()
                .route("/", get(site::home))
                .route("/signup", get(site::signup_page).post(site::signup_submit))
                .route("/verify", get(site::verify))
                .route("/verify/resend", post(site::resend_verification))
                .route("/login", get(site::login_page).post(site::login_submit))
                .route("/account", get(site::account_page))
                .route("/account/conversations", get(site::conversations_page))
                .route("/account/conversations/{id}", get(site::conversation_page))
                .route("/connections", post(site::create_connection))
                .route("/connections/revoke", post(site::revoke_connection))
                .route("/organizations", post(site::create_organization))
                .route("/invitations", post(site::send_invitation))
                .route(
                    "/invitations/accept",
                    get(site::invitation_page).post(site::accept_invitation),
                )
                .route("/logout", post(site::logout)),
        );
        app = app.merge(
            Router::new()
                .route(
                    "/.well-known/oauth-protected-resource",
                    get(oauth::resource_metadata),
                )
                .route(
                    "/.well-known/oauth-protected-resource/mcp",
                    get(oauth::resource_metadata),
                )
                .route(
                    "/.well-known/oauth-authorization-server",
                    get(oauth::authorization_metadata),
                )
                .route("/oauth/register", post(oauth::register_client))
                .route(
                    "/oauth/authorize",
                    get(oauth::authorize_page).post(oauth::authorize_submit),
                )
                .route("/oauth/token", post(oauth::token)),
        );
    }

    if state.mcp_principal.is_some() || state.account_mcp_enabled {
        let services = state.services.clone();
        let service: StreamableHttpService<ownstate_mcp::OwnstateMcp, LocalSessionManager> =
            StreamableHttpService::new(
                move || {
                    Ok(ownstate_mcp::OwnstateMcp::authenticated_http(
                        services.clone(),
                    ))
                },
                Default::default(),
                StreamableHttpServerConfig::default()
                    .with_allowed_hosts(state.mcp_allowed_hosts.clone())
                    .with_max_request_body_bytes(MAX_BODY_BYTES),
            );
        let mcp =
            Router::new()
                .nest_service("/mcp", service)
                .layer(middleware::from_fn_with_state(
                    state.clone(),
                    require_mcp_bearer,
                ));
        app = app.merge(mcp);
    }

    app.layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        // Span carries method + path only: query strings hold user text
        // (e.g. /knowledge/search?q=...) which must not reach logs.
        .layer(TraceLayer::new_for_http().make_span_with(
            |request: &axum::http::Request<axum::body::Body>| {
                tracing::info_span!(
                    "http",
                    method = %request.method(),
                    path = %request.uri().path(),
                )
            },
        ))
        .with_state(state)
}
