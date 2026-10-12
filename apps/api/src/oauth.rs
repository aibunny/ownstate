//! Public OAuth endpoints for account-owned, refreshable MCP connections.
use axum::Json;
use axum::extract::{Form, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;
use serde_json::json;

use crate::site::{escape_html, page, same_origin, web_token};
use crate::state::AppState;

fn base(state: &AppState) -> Option<&str> {
    state.signup_config.as_ref().map(|c| c.public_url.as_str())
}

pub async fn resource_metadata(State(state): State<AppState>) -> Response {
    let Some(base) = base(&state) else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    Json(json!({
        "resource": format!("{base}/mcp"),
        "authorization_servers": [base],
        "bearer_methods_supported": ["header"],
        "scopes_supported": ["ownstate:access", "offline_access"]
    }))
    .into_response()
}

pub async fn authorization_metadata(State(state): State<AppState>) -> Response {
    let Some(base) = base(&state) else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    Json(json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/oauth/authorize"),
        "token_endpoint": format!("{base}/oauth/token"),
        "registration_endpoint": format!("{base}/oauth/register"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["none"],
        "scopes_supported": ["ownstate:access", "offline_access"]
    }))
    .into_response()
}

#[derive(Deserialize)]
pub struct ClientRegistration {
    client_name: String,
    redirect_uris: Vec<String>,
    token_endpoint_auth_method: Option<String>,
}

pub async fn register_client(
    State(state): State<AppState>,
    Json(input): Json<ClientRegistration>,
) -> Response {
    if state.signup_config.is_none() {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    if input
        .token_endpoint_auth_method
        .as_deref()
        .is_some_and(|v| v != "none")
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"invalid_client_metadata"})),
        )
            .into_response();
    }
    match state
        .services
        .register_oauth_client(&input.client_name, input.redirect_uris)
        .await
    {
        Ok(client) => (
            StatusCode::CREATED,
            Json(json!({
                "client_id": client.client_id,
                "client_name": client.client_name,
                "redirect_uris": client.redirect_uris,
                "token_endpoint_auth_method": "none",
                "grant_types": ["authorization_code", "refresh_token"],
                "response_types": ["code"]
            })),
        )
            .into_response(),
        Err(_) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"invalid_redirect_uri"})),
        )
            .into_response(),
    }
}

#[derive(Clone, Deserialize)]
pub struct AuthorizationInput {
    response_type: String,
    client_id: String,
    redirect_uri: String,
    code_challenge: String,
    code_challenge_method: String,
    state: Option<String>,
    scope: Option<String>,
    resource: Option<String>,
    workspace_id: Option<String>,
}

fn authorize_input_valid(input: &AuthorizationInput, base: &str) -> bool {
    input.response_type == "code"
        && input.code_challenge_method == "S256"
        && input.client_id.len() <= 200
        && input.redirect_uri.len() <= 2048
        && input.code_challenge.len() == 43
        && input.state.as_deref().is_none_or(|s| s.len() <= 2048)
        && input.scope.as_deref().is_none_or(|s| {
            s.split_whitespace()
                .all(|v| matches!(v, "ownstate:access" | "offline_access"))
        })
        && input
            .resource
            .as_deref()
            .is_none_or(|r| r.is_empty() || r == format!("{base}/mcp"))
}

pub async fn authorize_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(input): Query<AuthorizationInput>,
) -> Response {
    let Some(base) = base(&state) else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    if !authorize_input_valid(&input, base) {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let Ok(client_name) = state
        .services
        .oauth_client(&input.client_id, &input.redirect_uri)
        .await
    else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let account = match web_token(&headers) {
        Some(token) => state.services.account_for_web_session(token).await.ok(),
        None => None,
    };
    let Some(account_id) = account else {
        let next = format!(
            "/oauth/authorize?{}",
            url::form_urlencoded::Serializer::new(String::new())
                .append_pair("response_type", &input.response_type)
                .append_pair("client_id", &input.client_id)
                .append_pair("redirect_uri", &input.redirect_uri)
                .append_pair("code_challenge", &input.code_challenge)
                .append_pair("code_challenge_method", &input.code_challenge_method)
                .append_pair("state", input.state.as_deref().unwrap_or(""))
                .append_pair("scope", input.scope.as_deref().unwrap_or(""))
                .append_pair("resource", input.resource.as_deref().unwrap_or(""))
                .finish()
        );
        return Redirect::to(&format!(
            "/login?next={}",
            url::form_urlencoded::byte_serialize(next.as_bytes()).collect::<String>()
        ))
        .into_response();
    };
    let hidden = [
        ("response_type", input.response_type.as_str()),
        ("client_id", input.client_id.as_str()),
        ("redirect_uri", input.redirect_uri.as_str()),
        ("code_challenge", input.code_challenge.as_str()),
        (
            "code_challenge_method",
            input.code_challenge_method.as_str(),
        ),
        ("state", input.state.as_deref().unwrap_or("")),
        ("scope", input.scope.as_deref().unwrap_or("")),
        ("resource", input.resource.as_deref().unwrap_or("")),
    ]
    .into_iter()
    .map(|(name, value)| {
        format!(
            "<input type=hidden name={name} value=\"{}\">",
            escape_html(value)
        )
    })
    .collect::<String>();
    let choices = match state.services.account_workspaces(account_id).await {
        Ok(choices) => choices,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let options = choices
        .into_iter()
        .map(|choice| {
            format!(
                "<label><input type=radio name=workspace_id value=\"{}\" required> {} ({})</label>",
                choice.workspace_id,
                escape_html(&choice.name),
                escape_html(&choice.kind)
            )
        })
        .collect::<String>();
    page("Authorize connection", &format!("<h1>Connect Ownstate</h1><p><strong>{}</strong> requests access to the Ownstate space you select. It can search context and record evidence through Ownstate tools.</p><form method=post action=/oauth/authorize>{hidden}<fieldset><legend>Choose a space</legend>{options}</fieldset><button>Allow connection</button></form><p><a href=/account>Cancel</a></p>", escape_html(&client_name))).into_response()
}

pub async fn authorize_submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(input): Form<AuthorizationInput>,
) -> Response {
    if !same_origin(&headers, &state) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(base) = base(&state) else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    if !authorize_input_valid(&input, base) {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let Some(web) = web_token(&headers) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(account) = state.services.account_for_web_session(web).await else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Some(workspace_id) = input
        .workspace_id
        .as_deref()
        .and_then(|s| uuid::Uuid::parse_str(s).ok())
    else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    match state
        .services
        .authorize_oauth(
            account,
            workspace_id,
            &input.client_id,
            &input.redirect_uri,
            &input.code_challenge,
        )
        .await
    {
        Ok(code) => {
            let Ok(mut redirect) = url::Url::parse(&input.redirect_uri) else {
                return StatusCode::BAD_REQUEST.into_response();
            };
            redirect.query_pairs_mut().append_pair("code", &code);
            if let Some(state_value) = input.state {
                redirect
                    .query_pairs_mut()
                    .append_pair("state", &state_value);
            }
            Redirect::to(redirect.as_str()).into_response()
        }
        Err(_) => StatusCode::BAD_REQUEST.into_response(),
    }
}

#[derive(Deserialize)]
pub struct TokenInput {
    grant_type: String,
    client_id: String,
    code: Option<String>,
    redirect_uri: Option<String>,
    code_verifier: Option<String>,
    refresh_token: Option<String>,
}

pub async fn token(State(state): State<AppState>, Form(input): Form<TokenInput>) -> Response {
    if state.signup_config.is_none() {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let result = match input.grant_type.as_str() {
        "authorization_code" => match (input.code, input.redirect_uri, input.code_verifier) {
            (Some(code), Some(redirect), Some(verifier)) => {
                state
                    .services
                    .exchange_oauth_code(&code, &input.client_id, &redirect, &verifier)
                    .await
            }
            _ => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error":"invalid_request"})),
                )
                    .into_response();
            }
        },
        "refresh_token" => match input.refresh_token {
            Some(refresh) => {
                state
                    .services
                    .refresh_oauth(&refresh, &input.client_id)
                    .await
            }
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error":"invalid_request"})),
                )
                    .into_response();
            }
        },
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error":"unsupported_grant_type"})),
            )
                .into_response();
        }
    };
    match result {
        Ok(pair) => Json(json!({
            "access_token": pair.access_token,
            "token_type": "Bearer",
            "expires_in": pair.expires_in,
            "refresh_token": pair.refresh_token,
            "scope": "ownstate:access offline_access"
        }))
        .into_response(),
        Err(_) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"invalid_grant"})),
        )
            .into_response(),
    }
}
