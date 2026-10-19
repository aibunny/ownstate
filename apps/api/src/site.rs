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
    let layout = match title {
        "Account" | "Saved chats" | "Saved chat" => "workspace",
        _ => "focused",
    };
    let body = if layout == "focused" {
        format!("<div class=auth-card>{body}</div>")
    } else {
        body.to_string()
    };
    Html(format!(
        r##"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><meta name="referrer" content="no-referrer"><meta name="theme-color" content="#080e0f"><title>{title} · Ownstate</title><link rel="preconnect" href="https://fonts.googleapis.com"><link rel="preconnect" href="https://fonts.gstatic.com" crossorigin><link href="https://fonts.googleapis.com/css2?family=DM+Mono:wght@400;500&family=Manrope:wght@400;500;600;700&family=Playfair+Display:ital,wght@0,500;0,600;1,500&display=swap" rel="stylesheet"><style>{css}</style></head><body class="{layout}"><header class="topbar"><a class="wordmark" href="/account" aria-label="Ownstate account"><span class="brand-mark" aria-hidden="true">{mark}</span><span>OWNSTATE</span></a><nav aria-label="Account navigation"><a href="/account">Overview</a><a href="/account/conversations">Saved chats</a></nav><span class="header-label">YOUR STATE, IN YOUR HANDS</span></header><main>{body}</main><footer class="site-footer"><span>OWNSTATE / YOUR CONTEXT LIVES HERE</span><span>Evidence stays connected to what you know.</span></footer></body></html>"##,
        css = include_str!("site.css"),
        mark = include_str!("site-mark.svg"),
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
            "<p class=eyebrow>Begin / Your own space</p><h1>Keep what you learn.</h1><p class=muted>Create an Ownstate account to carry your context across supported AI apps.</p><form method=post action=/signup><input type=hidden name=next value=\"{}\"><label>Email<input type=email name=email autocomplete=email required></label><label>Password (at least 12 characters)<input type=password name=password minlength=12 autocomplete=new-password required></label><button>Create account <span aria-hidden=true>↗</span></button></form><div class=secondary-form><h2>Need another verification link?</h2><form method=post action=/verify/resend><label>Email<input type=email name=email autocomplete=email required></label><button class=button-quiet>Resend link</button></form></div><p class=foot-link>Already have an account? <a href=/login>Sign in</a></p>",
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
            "<p class=eyebrow>Welcome back / Ownstate</p><h1>Return to your state.</h1><p class=muted>Sign in to see your spaces, connected apps, and saved chats.</p><form method=post action=/login><input type=hidden name=next value=\"{}\"><label>Email<input type=email name=email autocomplete=email required></label><label>Password<input type=password name=password autocomplete=current-password required></label><button>Sign in <span aria-hidden=true>↗</span></button></form><p class=foot-link>New to Ownstate? <a href=/signup?next={signup_next}>Create an account</a></p>",
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
        .map(|space| format!("<li class=org-item><strong>{}</strong><form method=post action=/invitations><input type=hidden name=workspace_id value=\"{}\"><label>Invite by email<input type=email name=email autocomplete=email placeholder='name@example.com' required></label><button class=button-quiet>Send invite</button></form></li>",escape_html(&space.name),space.workspace_id)).collect::<String>();
    let organization_count = workspaces
        .iter()
        .filter(|space| space.kind == "ORGANIZATION")
        .count();
    let organization_rows = if organizations.is_empty() {
        "<p class=empty-state>No organizations yet. Create one to share a separate space with your team.</p>".to_string()
    } else {
        format!("<ul class=list-clean>{organizations}</ul>")
    };
    let Ok(connections) = state.services.account_connections(account).await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let connection_rows = connections.iter().map(|c| {
        let kind = if c.oauth_grant_id.is_some() { "Signed in" } else { "Manual token" };
        format!("<li class=connection-row><div><strong>{}</strong><small>{} · {kind} · connected {}</small></div><form method=post action=/connections/revoke><input type=hidden name=connection_id value=\"{}\"><button class=button-danger aria-label='Revoke {} connection'>Revoke</button></form></li>", escape_html(&c.client_name), escape_html(&c.workspace_name), c.created_at.format("%Y-%m-%d"), c.id, escape_html(&c.client_name))
    }).collect::<String>();
    let connection_rows = if connection_rows.is_empty() {
        "<p class=empty-state>No apps connected yet. Use the address above to connect an app.</p>"
            .to_string()
    } else {
        format!("<ul class=list-clean>{connection_rows}</ul>")
    };
    let mcp_url = state
        .signup_config
        .as_ref()
        .map(|config| format!("{}/mcp", config.public_url))
        .unwrap_or_default();
    page("Account", &format!(r#"
        <section class="workspace-hero"><div><p class="eyebrow">Your Ownstate workspace</p><h1>Keep your context. <em>Change your tools.</em></h1><p class="supporting">Your saved knowledge and conversations live in the space you choose, ready for the AI apps you connect.</p></div><a class="primary-link" href="/account/conversations">View saved chats <span aria-hidden="true">↗</span></a></section>
        <div class="summary-strip" aria-label="Workspace overview"><div class="summary-item"><strong>{space_count}</strong><span>Your spaces</span></div><div class="summary-item"><strong>{connection_count}</strong><span>Connections</span></div><div class="summary-item"><strong>{organization_count}</strong><span>Organizations</span></div></div>
        <div class="dashboard-grid"><section class="panel"><p class="section-label">Connect / OAuth</p><h2>Connect an AI app</h2><p class="supporting">Add this MCP address in a supported app, then sign in with Ownstate. Choose your Personal space or an organization. The sign-in stays connected across sessions.</p><code class="connection-address">{mcp_url}</code><ol class="steps"><li><span>01 / ADD</span>Use the address above in your app's connector settings.</li><li><span>02 / SIGN IN</span>Sign in to Ownstate when the app asks.</li><li><span>03 / CHOOSE</span>Select the space the app can access.</li></ol><p class="note">In Claude, add a custom Web connector. The connector can use only the Ownstate tools you approve. Complete chat history is not captured automatically.</p></section>
        <section class="panel"><div class="panel-header"><div><p class="section-label">Continuity / Chats</p><h2>Saved chats</h2></div><a href="/account/conversations">Explore ↗</a></div><p class="supporting">See the conversation events that connected clients have sent to Ownstate. Each chat keeps its source and space visible.</p><p class="empty-state">Clients may send only part of a conversation. Open saved chats to see exactly what was recorded.</p></section></div>
        <div class="manage-grid"><section class="panel"><p class="section-label">Access / Apps</p><h2>Connections</h2><p class="supporting">See signed-in apps and manual tokens here. Revoke any connection when it no longer needs access.</p><h3 class="subsection-heading">Active connections</h3>{connection_rows}<div class="org-create"><h3>Create a manual connection</h3><p class="supporting">For local adapters or clients without Ownstate sign-in. Keep the token private.</p><form class="form-row" method=post action=/connections><label>Client name<input name=client_name placeholder='Local adapter name' maxlength=100 required></label><label>Ownstate space<select name=workspace_id required>{options}</select></label><button>Create connection</button></form></div></section>
        <section class="panel"><p class="section-label">Spaces / Teams</p><h2>Organizations</h2><p class="supporting">Each organization has its own space. Invite collaborators only to the one they should use.</p>{organization_rows}<div class="org-create"><h3>Create an organization</h3><form method=post action=/organizations><label>Organization name<input name=name maxlength=200 placeholder='Your team' required></label><button>Create</button></form></div></section></div>
        <div class="account-actions"><form method=post action=/logout><button class=button-quiet>Sign out</button></form></div>
    "#,
        space_count = workspaces.len(),
        connection_count = connections.len(),
        mcp_url = escape_html(&mcp_url),
    )).into_response()
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
                "<li><a href=/account/conversations/{}><strong>{}</strong><span aria-hidden=true>↗</span><small>{} / {} / {}</small></a></li>",
                c.id,
                escape_html(c.title.as_deref().unwrap_or("Untitled chat")),
                escape_html(&c.source),
                escape_html(&c.space),
                c.created_at.format("%Y-%m-%d")
            )
        })
        .collect::<String>();
    let rows = if rows.is_empty() {
        "<p class=empty-state>No chats saved yet. When a connected app sends conversation events, they will appear here.</p>".to_string()
    } else {
        format!("<ul class=chat-list>{rows}</ul>")
    };
    page("Saved chats", &format!("<section class=workspace-hero><div><p class=eyebrow>Continuity / Conversations</p><h1>Saved <em>chats.</em></h1><p class=supporting>These are the events connected clients sent to Ownstate. Some clients send only part of a chat.</p></div></section>{rows}<a class=back-link href=/account>← Back to overview</a>")).into_response()
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
                "<article class=event-card><strong>{}</strong><p>{}</p></article>",
                escape_html(&e.actor_type),
                escape_html(e.content.as_deref().unwrap_or(""))
            )
        })
        .collect::<String>();
    page("Saved chat", &format!("<section class=workspace-hero><div><p class=eyebrow>Saved chat / {}</p><h1>{}</h1><p class=supporting>Space: {}</p></div></section><div class=event-list>{rows}</div><a class=back-link href=/account/conversations>← All saved chats</a>",
        escape_html(&thread.source),
        escape_html(thread.title.as_deref().unwrap_or("Untitled chat")), escape_html(&thread.space))).into_response()
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
        Err(ownstate_services::ServiceError::Forbidden(_)) => (
            StatusCode::CONFLICT,
            page(
                "Space changed",
                "<h1>Your space changed</h1><p>Reload your account page and create this connection again.</p><p><a href=/account>Return to account</a></p>",
            ),
        )
            .into_response(),
        Err(ownstate_services::ServiceError::Validation(_)) => StatusCode::BAD_REQUEST.into_response(),
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
