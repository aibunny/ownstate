use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::middleware;
use axum::routing::{get, post};
use tower_http::trace::TraceLayer;

use crate::auth::require_bearer;
use crate::handlers;
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

    Router::new()
        .route("/health", get(handlers::health))
        .route("/ready", get(handlers::ready))
        .merge(protected)
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
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
