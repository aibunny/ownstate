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
        Some(digest) => {
            if state
                .services
                .is_mcp_credential(&digest)
                .await
                .unwrap_or(false)
            {
                None
            } else {
                state.services.authenticate_digest(&digest).await.ok()
            }
        }
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

/// Require the dedicated MCP credential on every Streamable HTTP request.
/// API and admin credentials are deliberately rejected at this boundary.
pub async fn require_mcp_bearer(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let expected = state.mcp_principal.as_ref().map(|principal| principal.id());
    let principal = match bearer_hash(request.headers()) {
        Some(digest) => {
            let principal = state.services.authenticate_digest(&digest).await.ok();
            let is_legacy = principal.as_ref().is_some_and(|p| Some(p.id()) == expected);
            let is_account = state.account_mcp_enabled
                && state
                    .services
                    .is_active_mcp_connection(&digest)
                    .await
                    .unwrap_or(false);
            if is_legacy || is_account {
                principal
            } else {
                None
            }
        }
        None => None,
    };
    match principal {
        Some(principal) => {
            request.extensions_mut().insert(principal);
            next.run(request).await
        }
        _ => {
            let mut response = (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                "error": { "code": "unauthorized", "message": "missing or invalid MCP bearer token" }
                })),
            ).into_response();
            if let Some(config) = &state.signup_config {
                let challenge = format!(
                    "Bearer resource_metadata=\"{}/.well-known/oauth-protected-resource/mcp\", scope=\"ownstate:access\"",
                    config.public_url,
                );
                if let Ok(value) = challenge.parse() {
                    response
                        .headers_mut()
                        .insert(header::WWW_AUTHENTICATE, value);
                }
            }
            response
        }
    }
}

/// Host adapters may append observable chat events using the account's MCP
/// connection. The connection is resolved on every request, including revoke.
pub async fn require_capture_bearer(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let Some(digest) = bearer_hash(request.headers()) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if !state.account_mcp_enabled
        || !state
            .services
            .is_active_mcp_connection(&digest)
            .await
            .unwrap_or(false)
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let Ok(principal) = state.services.authenticate_digest(&digest).await else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    request.extensions_mut().insert(principal);
    next.run(request).await
}
