//! Small account website. Identity and credential mutations remain outside MCP.
use axum::extract::{Form, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

use crate::state::AppState;

pub struct SignupConfig {
    pub(crate) public_url: String,
    pub(crate) origin: String,
    delivery: EmailDelivery,
}

enum EmailDelivery {
    Smtp {
        host: String,
        port: u16,
        username: String,
        password: String,
        from: String,
    },
    Resend {
        api_key: String,
        from: String,
    },
}

impl SignupConfig {
    /// Returns None until a public URL and one mail delivery method are configured.
    pub fn from_env() -> Result<Option<Self>, String> {
        let public_url = match std::env::var("OWNSTATE_PUBLIC_URL") {
            Ok(v) if !v.trim().is_empty() => v.trim_end_matches('/').to_string(),
            _ => return Ok(None),
        };
        let resend_api_key = std::env::var("OWNSTATE_RESEND_API_KEY")
            .ok()
            .filter(|value| !value.is_empty());
        let smtp_password = std::env::var("OWNSTATE_SMTP_PASSWORD")
            .ok()
            .filter(|value| !value.is_empty());
        if resend_api_key.is_none() && smtp_password.is_none() {
            return Ok(None);
        }
        let url = url::Url::parse(&public_url).map_err(|_| "OWNSTATE_PUBLIC_URL is invalid")?;
        if url.scheme() != "https" && url.host_str() != Some("localhost") {
            return Err("OWNSTATE_PUBLIC_URL must use HTTPS".into());
        }
        if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
            return Err("OWNSTATE_PUBLIC_URL must be an origin without a path".into());
        }
        let origin = url.origin().ascii_serialization();
        let from = std::env::var("OWNSTATE_SMTP_FROM")
            .unwrap_or_else(|_| "ownstate@theaibunny.com".into());
        let delivery = if let Some(api_key) = resend_api_key {
            EmailDelivery::Resend { api_key, from }
        } else if let Some(password) = smtp_password {
            let port = std::env::var("OWNSTATE_SMTP_PORT")
                .unwrap_or_else(|_| "587".into())
                .parse::<u16>()
                .map_err(|_| "OWNSTATE_SMTP_PORT is invalid")?;
            if port == 0 {
                return Err("OWNSTATE_SMTP_PORT is invalid".into());
            }
            let username = std::env::var("OWNSTATE_SMTP_USERNAME").unwrap_or_else(|_| from.clone());
            EmailDelivery::Smtp {
                host: std::env::var("OWNSTATE_SMTP_HOST")
                    .unwrap_or_else(|_| "smtp.larksuite.com".into()),
                port,
                username,
                password,
                from,
            }
        } else {
            return Ok(None);
        };
        Ok(Some(Self {
            public_url,
            origin,
            delivery,
        }))
    }

    fn send_email(&self, recipient: &str, subject: &str, body: String) -> Result<(), String> {
        let to: Mailbox = recipient.parse().map_err(|_| "invalid recipient")?;
        match &self.delivery {
            EmailDelivery::Smtp {
                host,
                port,
                username,
                password,
                from,
            } => {
                let from: Mailbox = from.parse().map_err(|_| "invalid SMTP sender")?;
                let message = Message::builder()
                    .from(from)
                    .to(to)
                    .subject(subject)
                    .body(body)
                    .map_err(|_| "could not build email")?;
                let transport = SmtpTransport::starttls_relay(host)
                    .map_err(|_| "SMTP configuration failed")?
                    .port(*port)
                    .credentials(Credentials::new(username.clone(), password.clone()))
                    .build();
                transport
                    .send(&message)
                    .map_err(|_| "SMTP delivery failed")?;
                Ok(())
            }
            EmailDelivery::Resend { api_key, from } => {
                let _: Mailbox = from.parse().map_err(|_| "invalid email sender")?;
                let config = ureq::Agent::config_builder()
                    .timeout_global(Some(Duration::from_secs(20)))
                    .build();
                let agent: ureq::Agent = config.into();
                let response = agent
                    .post("https://api.resend.com/emails")
                    .header("Authorization", format!("Bearer {api_key}"))
                    .send_json(json!({
                        "from": from,
                        "to": [to.email.to_string()],
                        "subject": subject,
                        "text": body,
                    }))
                    .map_err(|_| "Resend delivery failed")?;
                if !response.status().is_success() {
                    return Err("Resend delivery failed".into());
                }
                Ok(())
            }
        }
    }

    fn send_verification(
        &self,
        recipient: &str,
        token: &str,
        next: Option<&str>,
    ) -> Result<(), String> {
        let mut url = url::Url::parse(&format!("{}/verify", self.public_url))
            .map_err(|_| "invalid public URL")?;
        url.query_pairs_mut()
            .append_pair("token", token)
            .append_pair("next", safe_next(next));
        self.send_email(recipient, "Verify your Ownstate account", format!("Open this link to verify your Ownstate account:\n\n{url}\n\nThis link expires in 24 hours.\n"))
    }

    fn send_invitation(&self, recipient: &str, token: &str) -> Result<(), String> {
        let url = format!("{}/invitations/accept?token={token}", self.public_url);
        self.send_email(recipient, "You are invited to an Ownstate organization", format!("Open this link to join an Ownstate organization:\n\n{url}\n\nSign in or create an account with this email address. The link expires in 7 days.\n"))
    }
}

pub(crate) fn page(title: &str, body: &str) -> Html<String> {
    Html(format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><meta name="referrer" content="no-referrer"><title>{title} · Ownstate</title><style>body{{font:16px/1.5 system-ui,sans-serif;background:#f5f4ef;color:#1f2724;margin:0}}main{{max-width:520px;margin:8vh auto;padding:2rem;background:white;border:1px solid #d5d9d1;border-radius:16px}}h1{{font-size:1.7rem}}label{{display:block;margin:1rem 0 .3rem}}input{{box-sizing:border-box;width:100%;padding:.75rem;border:1px solid #89948a;border-radius:8px;font:inherit}}button{{margin-top:1.25rem;padding:.75rem 1.2rem;border:0;border-radius:8px;background:#174f3c;color:white;font:inherit;cursor:pointer}}a{{color:#174f3c}}code{{overflow-wrap:anywhere}}.muted{{color:#5c675e}}</style></head><body><main>{body}</main></body></html>"#
    ))
}

pub(crate) fn same_origin(headers: &HeaderMap, state: &AppState) -> bool {
    let Some(config) = &state.signup_config else {
        return false;
    };
    request_from_same_origin(headers, &config.origin)
}

fn request_from_same_origin(headers: &HeaderMap, expected_origin: &str) -> bool {
    let supplied_origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok());
    if supplied_origin == Some(expected_origin) {
        return true;
    }
    // Some browsers omit Origin (or send "null") for a form submitted from a
    // page with a no-referrer policy. Fetch Metadata still identifies a real
    // same-origin browser navigation; the Host check binds it to this site.
    if supplied_origin.is_some_and(|origin| origin != "null") {
        return false;
    }
    let Some((_, expected_host)) = expected_origin.split_once("://") else {
        return false;
    };
    headers.get("sec-fetch-site").and_then(|v| v.to_str().ok()) == Some("same-origin")
        && headers
            .get(header::HOST)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|host| host.eq_ignore_ascii_case(expected_host))
}

#[cfg(test)]
mod origin_tests {
    use super::request_from_same_origin;
    use axum::http::{HeaderMap, HeaderValue, header};

    const ORIGIN: &str = "https://api-production-8889.up.railway.app";

    #[test]
    fn accepts_matching_origin_or_same_origin_browser_metadata() {
        let mut headers = HeaderMap::new();
        headers.insert(header::ORIGIN, HeaderValue::from_static(ORIGIN));
        assert!(request_from_same_origin(&headers, ORIGIN));

        headers.remove(header::ORIGIN);
        headers.insert("sec-fetch-site", HeaderValue::from_static("same-origin"));
        headers.insert(
            header::HOST,
            HeaderValue::from_static("api-production-8889.up.railway.app"),
        );
        assert!(request_from_same_origin(&headers, ORIGIN));

        headers.insert(header::ORIGIN, HeaderValue::from_static("null"));
        assert!(request_from_same_origin(&headers, ORIGIN));
    }

    #[test]
    fn rejects_cross_site_or_unidentified_requests() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::HOST,
            HeaderValue::from_static("api-production-8889.up.railway.app"),
        );
        assert!(!request_from_same_origin(&headers, ORIGIN));

        headers.insert("sec-fetch-site", HeaderValue::from_static("cross-site"));
        assert!(!request_from_same_origin(&headers, ORIGIN));

        headers.insert("sec-fetch-site", HeaderValue::from_static("same-origin"));
        headers.insert(header::HOST, HeaderValue::from_static("attacker.example"));
        assert!(!request_from_same_origin(&headers, ORIGIN));

        headers.insert(
            header::HOST,
            HeaderValue::from_static("api-production-8889.up.railway.app"),
        );
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://attacker.example"),
        );
        assert!(!request_from_same_origin(&headers, ORIGIN));
    }
}

pub(crate) fn web_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|cookie| {
            let (name, value) = cookie.trim().split_once('=')?;
            (name == "__Host-ownstate_session" || name == "ownstate_session").then_some(value)
        })
}

#[derive(Deserialize)]
pub struct SignupForm {
    email: String,
    password: String,
    next: Option<String>,
}
#[derive(Deserialize)]
pub struct ResendForm {
    email: String,
}
#[derive(Deserialize)]
pub struct RevokeConnectionForm {
    connection_id: String,
}
#[derive(Deserialize)]
pub struct LoginForm {
    email: String,
    password: String,
    next: Option<String>,
}
#[derive(Deserialize)]
pub struct LoginNext {
    next: Option<String>,
}
fn safe_next(next: Option<&str>) -> &str {
    match next {
        Some(value)
            if (value.starts_with("/oauth/authorize?")
                || value.starts_with("/invitations/accept?"))
                && !value.contains(['\r', '\n']) =>
        {
            value
        }
        _ => "/account",
    }
}
pub(crate) fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
#[derive(Deserialize)]
pub struct VerifyQuery {
    token: String,
    next: Option<String>,
}
#[derive(Deserialize)]
pub struct ConnectionForm {
    client_name: String,
    workspace_id: String,
}
#[derive(Deserialize)]
pub struct OrganizationForm {
    name: String,
}
#[derive(Deserialize)]
pub struct InvitationForm {
    workspace_id: String,
    email: String,
}
#[derive(Deserialize)]
pub struct InvitationQuery {
    token: String,
}
#[derive(Deserialize)]
pub struct AcceptInvitationForm {
    token: String,
}

pub async fn home() -> impl IntoResponse {
    Redirect::to("/signup")
}

pub async fn signup_page(
    State(state): State<AppState>,
    Query(next): Query<LoginNext>,
) -> impl IntoResponse {
    if state.signup_config.is_none() {
        return page(
            "Sign up",
            "<h1>Signups are being configured</h1><p>Check back after email delivery is enabled.</p>",
        );
    }
    page(
        "Sign up",
        &format!(
            "<h1>Create your Ownstate account</h1><p class=muted>Your personal memory stays connected across supported AI sessions.</p><form method=post action=/signup><input type=hidden name=next value=\"{}\"><label>Email</label><input type=email name=email autocomplete=email required><label>Password (at least 12 characters)</label><input type=password name=password minlength=12 autocomplete=new-password required><button>Create account</button></form><h2>Need a new verification link?</h2><form method=post action=/verify/resend><label>Email</label><input type=email name=email required><button>Resend link</button></form><p><a href=/login>Already have an account?</a></p>",
            escape_html(safe_next(next.next.as_deref()))
        ),
    )
}

pub async fn resend_verification(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<ResendForm>,
) -> Response {
    if !same_origin(&headers, &state) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(config) = state.signup_config else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    match state.services.resend_verification(&form.email).await {
        Ok(Some(pending)) => {
            let result = tokio::task::spawn_blocking(move || {
                config.send_verification(&pending.email, &pending.verification_token, None)
            })
            .await;
            if !matches!(result, Ok(Ok(()))) {
                tracing::error!("verification resend failed");
                return StatusCode::SERVICE_UNAVAILABLE.into_response();
            }
        }
        Ok(None) => {}
        Err(e) => {
            tracing::error!(error=%e,"verification resend failed");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    }
    page("Check your email", "<h1>Check your email</h1><p>If a verification link is available, we sent it to that address.</p>").into_response()
}

pub async fn signup_submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<SignupForm>,
) -> Response {
    if !same_origin(&headers, &state) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(config) = state.signup_config else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    match state.services.signup(&form.email, &form.password).await {
        Ok(pending) => {
            let config = config.clone();
            let next = safe_next(form.next.as_deref()).to_string();
            let result = tokio::task::spawn_blocking(move || config.send_verification(&pending.email, &pending.verification_token, Some(&next))).await;
            match result {
                Ok(Ok(())) => page("Check your email", "<h1>Check your email</h1><p>We sent a verification link. Open it before signing in.</p>").into_response(),
                _ => { tracing::error!("signup verification email failed"); (StatusCode::SERVICE_UNAVAILABLE, page("Email unavailable", "<h1>Email delivery is unavailable</h1><p>Please try again later.</p>")).into_response() }
            }
        }
        Err(ownstate_services::ServiceError::Conflict(_)) => page("Check your email", "<h1>Check your email</h1><p>If this account can be created, a verification email will arrive.</p>").into_response(),
        Err(ownstate_services::ServiceError::Validation(_)) => (StatusCode::BAD_REQUEST, page("Invalid signup", "<h1>Check your details</h1><p>Use a valid email and a password of at least 12 characters.</p>")).into_response(),
        Err(e) => { tracing::error!(error=%e,"signup failed"); StatusCode::INTERNAL_SERVER_ERROR.into_response() }
    }
}

pub async fn verify(State(state): State<AppState>, Query(query): Query<VerifyQuery>) -> Response {
    match state.services.verify_signup(&query.token).await {
        Ok(()) => {
            let next =
                url::form_urlencoded::byte_serialize(safe_next(query.next.as_deref()).as_bytes())
                    .collect::<String>();
            page("Verified", &format!("<h1>Email verified</h1><p><a href=/login?next={next}>Sign in to Ownstate</a>.</p>")).into_response()
        }
        Err(_) => (
            StatusCode::BAD_REQUEST,
            page("Invalid link", "<h1>This link is invalid or expired</h1>"),
        )
            .into_response(),
    }
}

pub async fn login_page(Query(next): Query<LoginNext>) -> impl IntoResponse {
    let signup_next =
        url::form_urlencoded::byte_serialize(safe_next(next.next.as_deref()).as_bytes())
            .collect::<String>();
    page(
        "Sign in",
        &format!(
            "<h1>Sign in to Ownstate</h1><form method=post action=/login><input type=hidden name=next value=\"{}\"><label>Email</label><input type=email name=email autocomplete=email required><label>Password</label><input type=password name=password autocomplete=current-password required><button>Sign in</button></form><p><a href=/signup?next={signup_next}>Create an account</a></p>",
            escape_html(safe_next(next.next.as_deref()))
        ),
    )
}

pub async fn login_submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<LoginForm>,
) -> Response {
    if !same_origin(&headers, &state) {
        return StatusCode::FORBIDDEN.into_response();
    }
    match state.services.login(&form.email, &form.password).await {
        Ok(session) => {
            let secure = state.signup_config.as_ref().is_some_and(|c| c.origin.starts_with("https://"));
            let name = if secure { "__Host-ownstate_session" } else { "ownstate_session" };
            let cookie = format!("{name}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000{}", session.token, if secure { "; Secure" } else { "" });
            let mut response = Redirect::to(safe_next(form.next.as_deref())).into_response();
            if let Ok(value) = cookie.parse() { response.headers_mut().insert(header::SET_COOKIE, value); }
            response
        }
        Err(_) => (StatusCode::UNAUTHORIZED, page("Sign in failed", "<h1>Sign in failed</h1><p>Check your email, password, and verification status.</p><p><a href=/login>Try again</a></p>")).into_response(),
    }
}

pub async fn account_page(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(token) = web_token(&headers) else {
        return Redirect::to("/login").into_response();
    };
    let Ok(account) = state.services.account_for_web_session(token).await else {
        return Redirect::to("/login").into_response();
    };
    let Ok(workspaces) = state.services.account_workspaces(account).await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let options = workspaces
        .iter()
        .map(|space| {
            format!(
                "<option value=\"{}\">{} ({})</option>",
                space.workspace_id,
                escape_html(&space.name),
                escape_html(&space.kind)
            )
        })
        .collect::<String>();
    let organizations = workspaces.iter().filter(|space| space.kind == "ORGANIZATION")
        .map(|space| format!("<li>{} <form method=post action=/invitations><input type=hidden name=workspace_id value=\"{}\"><label>Invite by email</label><input type=email name=email required><button>Send invitation</button></form></li>",escape_html(&space.name),space.workspace_id)).collect::<String>();
    let Ok(connections) = state.services.account_connections(account).await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let connection_rows = connections.iter().map(|c| format!("<li><strong>{}</strong> · {} · {}<form method=post action=/connections/revoke><input type=hidden name=connection_id value=\"{}\"><button>Revoke</button></form></li>", escape_html(&c.client_name), escape_html(&c.workspace_name), c.created_at.format("%Y-%m-%d"), c.id)).collect::<String>();
    let mcp_url = state
        .signup_config
        .as_ref()
        .map(|config| format!("{}/mcp", config.public_url))
        .unwrap_or_default();
    page("Account", &format!("<h1>Your Ownstate account</h1><p><a href=/account/conversations>View saved chats</a></p><h2>Connect an AI app</h2><p>Add this remote MCP address in a supported client: <code>{}</code>. Sign in with Ownstate and choose your personal or organization space. Your OAuth connection is saved across sessions and can be revoked here. Connect separately for each space you want available.</p><p class=muted>The remote connector receives only the Ownstate tools you use. To save supported local chat events automatically, install a local capture adapter.</p><h2>Manual connection for local adapters</h2><p>Create a scoped token for a local capture adapter or a client that cannot sign in with OAuth. Keep the token private.</p><form method=post action=/connections><label>Client name</label><input name=client_name placeholder='Codex, Claude, or OpenCode' maxlength=100 required><label>Ownstate space</label><select name=workspace_id required>{options}</select><button>Create connection</button></form><h2>Active connections</h2><ul>{connection_rows}</ul><h2>Organizations</h2><ul>{organizations}</ul><form method=post action=/organizations><label>New organization name</label><input name=name maxlength=200 required><button>Create organization</button></form><form method=post action=/logout><button>Sign out</button></form>", escape_html(&mcp_url))).into_response()
}

pub async fn conversations_page(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(web) = web_token(&headers) else {
        return Redirect::to("/login").into_response();
    };
    let Ok(account) = state.services.account_for_web_session(web).await else {
        return Redirect::to("/login").into_response();
    };
    let Ok(conversations) = state.services.account_conversations(account).await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let rows = conversations
        .iter()
        .map(|c| {
            format!(
                "<li><a href=/account/conversations/{}>{}</a> · {} · {} · {}</li>",
                c.id,
                escape_html(c.title.as_deref().unwrap_or("Untitled chat")),
                escape_html(&c.source),
                escape_html(&c.space),
                c.created_at.format("%Y-%m-%d")
            )
        })
        .collect::<String>();
    page("Saved chats", &format!("<h1>Saved chats</h1><p class=muted>These are the events each client sent to Ownstate. Some clients send only part of a chat.</p><ul>{rows}</ul><p><a href=/account>Back to account</a></p>")).into_response()
}

pub async fn conversation_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
) -> Response {
    let Some(web) = web_token(&headers) else {
        return Redirect::to("/login").into_response();
    };
    let Ok(account) = state.services.account_for_web_session(web).await else {
        return Redirect::to("/login").into_response();
    };
    let Ok(conversations) = state.services.account_conversations(account).await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let Some(thread) = conversations.iter().find(|c| c.id == id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(events) = state
        .services
        .account_conversation_events(account, id)
        .await
    else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let rows = events
        .iter()
        .map(|e| {
            format!(
                "<article><strong>{}</strong><p style='white-space:pre-wrap'>{}</p></article>",
                escape_html(&e.actor_type),
                escape_html(e.content.as_deref().unwrap_or(""))
            )
        })
        .collect::<String>();
    page("Saved chat", &format!("<h1>{}</h1><p class=muted>{} · {}</p>{rows}<p><a href=/account/conversations>All saved chats</a></p>",
        escape_html(thread.title.as_deref().unwrap_or("Untitled chat")), escape_html(&thread.source), escape_html(&thread.space))).into_response()
}

pub async fn revoke_connection(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<RevokeConnectionForm>,
) -> Response {
    if !same_origin(&headers, &state) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(web) = web_token(&headers) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(account) = state.services.account_for_web_session(web).await else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(connection_id) = uuid::Uuid::parse_str(&form.connection_id) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    match state
        .services
        .revoke_account_connection(account, connection_id)
        .await
    {
        Ok(true) => Redirect::to("/account").into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!(error=%e,"connection revocation failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn create_connection(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<ConnectionForm>,
) -> Response {
    if !same_origin(&headers, &state) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(web) = web_token(&headers) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(account) = state.services.account_for_web_session(web).await else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Some(workspace_id) = uuid::Uuid::parse_str(&form.workspace_id).ok() else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    match state
        .services
        .create_mcp_connection(account, workspace_id, &form.client_name)
        .await
    {
        Ok(connection) => {
            let mcp_url = state
                .signup_config
                .as_ref()
                .map(|c| format!("{}/mcp", c.public_url))
                .unwrap_or_default();
            page("Connection created", &format!("<h1>Connection created</h1><p>Copy this token now. It will not be shown again.</p><p><code>{}</code></p><p>MCP server: <code>{mcp_url}</code></p><p>Send the token as <code>Authorization: Bearer &lt;token&gt;</code> when configuring your client.</p><p><a href=/account>Back to account</a></p>", connection.token)).into_response()
        }
        Err(e) => {
            tracing::error!(error=%e,"connection provisioning failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn create_organization(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<OrganizationForm>,
) -> Response {
    if !same_origin(&headers, &state) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(web) = web_token(&headers) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(account) = state.services.account_for_web_session(web).await else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    match state
        .services
        .create_organization(account, &form.name)
        .await
    {
        Ok(_) => Redirect::to("/account").into_response(),
        Err(ownstate_services::ServiceError::Validation(_)) => {
            StatusCode::BAD_REQUEST.into_response()
        }
        Err(e) => {
            tracing::error!(error=%e,"organization creation failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn send_invitation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<InvitationForm>,
) -> Response {
    if !same_origin(&headers, &state) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(config) = state.signup_config else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let Some(web) = web_token(&headers) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(account) = state.services.account_for_web_session(web).await else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Some(workspace) = uuid::Uuid::parse_str(&form.workspace_id).ok() else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    match state
        .services
        .invite_to_organization(account, workspace, &form.email)
        .await
    {
        Ok(invite) => {
            let result = tokio::task::spawn_blocking(move || {
                config.send_invitation(&invite.email, &invite.token)
            })
            .await;
            match result {
                Ok(Ok(())) => page(
                    "Invitation sent",
                    "<h1>Invitation sent</h1><p><a href=/account>Back to account</a></p>",
                )
                .into_response(),
                _ => {
                    tracing::error!("invitation email failed");
                    StatusCode::SERVICE_UNAVAILABLE.into_response()
                }
            }
        }
        Err(ownstate_services::ServiceError::Forbidden(_)) => StatusCode::FORBIDDEN.into_response(),
        Err(ownstate_services::ServiceError::Validation(_)) => {
            StatusCode::BAD_REQUEST.into_response()
        }
        Err(e) => {
            tracing::error!(error=%e,"invitation creation failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn invitation_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<InvitationQuery>,
) -> Response {
    let signed_in = web_token(&headers).is_some_and(|token| {
        // The session is checked again on submission. Avoid a database read
        // here solely to decide whether to render the confirmation form.
        !token.is_empty()
    });
    if !signed_in {
        let next = format!("/invitations/accept?token={}", query.token);
        return Redirect::to(&format!(
            "/login?next={}",
            url::form_urlencoded::byte_serialize(next.as_bytes()).collect::<String>()
        ))
        .into_response();
    }
    let _ = state;
    page("Join organization", &format!("<h1>Join this Ownstate organization</h1><p>Confirm with the account that received the invitation.</p><form method=post action=/invitations/accept><input type=hidden name=token value=\"{}\"><button>Join organization</button></form>",escape_html(&query.token))).into_response()
}

pub async fn accept_invitation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<AcceptInvitationForm>,
) -> Response {
    if !same_origin(&headers, &state) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(web) = web_token(&headers) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(account) = state.services.account_for_web_session(web).await else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    match state
        .services
        .accept_organization_invitation(account, &form.token)
        .await
    {
        Ok(()) => page(
            "Joined organization",
            "<h1>You joined the organization</h1><p><a href=/account>See your spaces</a></p>",
        )
        .into_response(),
        Err(_) => (
            StatusCode::BAD_REQUEST,
            page(
                "Invitation unavailable",
                "<h1>This invitation is invalid or expired</h1>",
            ),
        )
            .into_response(),
    }
}

pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !same_origin(&headers, &state) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if let Some(token) = web_token(&headers) {
        let _ = state.services.logout(token).await;
    }
    let mut response = Redirect::to("/login").into_response();
    if let Ok(cookie) =
        "__Host-ownstate_session=; Path=/; HttpOnly; SameSite=Lax; Secure; Max-Age=0".parse()
    {
        response.headers_mut().insert(header::SET_COOKIE, cookie);
    }
    response
}
