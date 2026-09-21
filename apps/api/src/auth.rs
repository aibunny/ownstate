use axum::Json;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::state::AppState;

fn bearer_hash(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|t| ownstate_domain::content_hash(t.as_bytes()))
}

/// Resolve bearer credentials into a server-owned principal on every request.
/// Revoked, expired, disabled, and unknown credentials all fail before a
/// handler can read resource identity or content.
pub async fn require_bearer(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let principal = match bearer_hash(request.headers()) {
        Some(digest) => state.services.authenticate_digest(&digest).await.ok(),
        None => state.personal_fallback.clone(),
    };
    match principal {
        Some(principal) => {
            request.extensions_mut().insert(principal);
            next.run(request).await
        }
        None => (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": { "code": "unauthorized", "message": "missing or invalid bearer token" }
            })),
        )
            .into_response(),
    }
}
