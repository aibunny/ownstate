//! OAuth authorization-code + PKCE and rotating refresh tokens for MCP clients.
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{Duration, Utc};
use ownstate_domain::content_hash;
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;

use crate::accounts::fresh_token;
use crate::{AppServices, ServiceError, ServiceResult};

pub struct RegisteredClient {
    pub client_id: String,
    pub client_name: String,
    pub redirect_uris: Vec<String>,
}

pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
}

impl AppServices {
    pub async fn register_oauth_client(
        &self,
        name: &str,
        redirect_uris: Vec<String>,
    ) -> ServiceResult<RegisteredClient> {
        self.check_auth_rate_limit("oauth-dcr", "deployment", 1000, 3600)
            .await?;
        let name = name.trim();
        if name.is_empty()
            || name.len() > 100
            || redirect_uris.is_empty()
            || redirect_uris.len() > 10
        {
            return Err(ServiceError::validation("invalid client registration"));
        }
        for uri in &redirect_uris {
            if uri.len() > 2048 {
                return Err(ServiceError::validation("redirect URI is too long"));
            }
            let url = url::Url::parse(uri)
                .map_err(|_| ServiceError::validation("invalid redirect URI"))?;
            let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
            if (url.scheme() != "https" && !(url.scheme() == "http" && loopback))
                || url.username() != ""
                || url.password().is_some()
                || url.fragment().is_some()
            {
                return Err(ServiceError::validation(
                    "redirect URI must use HTTPS or HTTP loopback",
                ));
            }
        }
        let client_id = fresh_token("ownstate_client_");
        sqlx::query(
            "INSERT INTO oauth_clients(client_id,client_name,redirect_uris) VALUES($1,$2,$3)",
        )
        .bind(&client_id)
        .bind(name)
        .bind(&redirect_uris)
        .execute(self.pool())
        .await?;
        Ok(RegisteredClient {
            client_id,
            client_name: name.to_string(),
            redirect_uris,
        })
    }

    pub async fn oauth_client(&self, client_id: &str, redirect_uri: &str) -> ServiceResult<String> {
        let name: Option<String> = sqlx::query_scalar(
            "SELECT client_name FROM oauth_clients WHERE client_id=$1 AND $2=ANY(redirect_uris)",
        )
        .bind(client_id)
        .bind(redirect_uri)
        .fetch_optional(self.pool())
        .await?;
        name.ok_or_else(|| ServiceError::validation("unknown client or redirect URI"))
    }

    pub async fn authorize_oauth(
        &self,
        account_id: Uuid,
        workspace_id: Uuid,
        client_id: &str,
        redirect_uri: &str,
        challenge: &str,
    ) -> ServiceResult<String> {
        if challenge.len() != 43
            || !challenge
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(ServiceError::validation("PKCE S256 challenge required"));
        }
        let mut tx = self.pool().begin().await?;
        let client: Option<(String,)> = sqlx::query_as(
            "SELECT client_id FROM oauth_clients WHERE client_id=$1 AND $2=ANY(redirect_uris)",
        )
        .bind(client_id)
        .bind(redirect_uri)
        .fetch_optional(&mut *tx)
        .await?;
        if client.is_none() {
            return Err(ServiceError::validation("unknown client or redirect URI"));
        }
        let owner: Option<(Uuid,Uuid)> = sqlx::query_as(
            "SELECT m.tenant_id,m.principal_id FROM account_tenant_memberships m \
             JOIN signup_accounts a ON a.id=m.account_id \
             JOIN workspaces w ON w.tenant_id=m.tenant_id AND w.id=m.workspace_id \
             WHERE m.account_id=$1 AND m.workspace_id=$2 AND a.email_verified_at IS NOT NULL \
             AND (w.personal_user_id=m.user_id OR EXISTS (SELECT 1 FROM organization_memberships om \
                 WHERE om.tenant_id=m.tenant_id AND om.organization_id=w.organization_id AND om.user_id=m.user_id)) \
             FOR SHARE OF m,a",
        ).bind(account_id).bind(workspace_id).fetch_optional(&mut *tx).await?;
        let (tenant_id, principal_id) =
            owner.ok_or_else(|| ServiceError::forbidden("verified account required"))?;
        let grant_id = Uuid::new_v4();
        sqlx::query("INSERT INTO oauth_grants(id,account_id,client_id,tenant_id,principal_id) VALUES($1,$2,$3,$4,$5)")
            .bind(grant_id).bind(account_id).bind(client_id).bind(tenant_id).bind(principal_id)
            .execute(&mut *tx).await?;
        let code = fresh_token("oc_");
        sqlx::query("INSERT INTO oauth_authorization_codes(code_digest,grant_id,redirect_uri,code_challenge,expires_at) VALUES($1,$2,$3,$4,$5)")
            .bind(content_hash(code.as_bytes())).bind(grant_id).bind(redirect_uri).bind(challenge)
            .bind(Utc::now()+Duration::minutes(5)).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(code)
    }

    pub async fn exchange_oauth_code(
        &self,
        code: &str,
        client_id: &str,
        redirect_uri: &str,
        verifier: &str,
    ) -> ServiceResult<TokenPair> {
        if verifier.len() < 43
            || verifier.len() > 128
            || !verifier
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~'))
        {
            return Err(ServiceError::forbidden("invalid PKCE verifier"));
        }
        let mut tx = self.pool().begin().await?;
        let row = sqlx::query(
            "SELECT c.grant_id,c.code_challenge,g.account_id,g.tenant_id,g.principal_id,cl.client_name \
             FROM oauth_authorization_codes c JOIN oauth_grants g ON g.id=c.grant_id \
             JOIN oauth_clients cl ON cl.client_id=g.client_id \
             JOIN signup_accounts a ON a.id=g.account_id \
             JOIN principals p ON p.id=g.principal_id AND p.tenant_id=g.tenant_id \
             WHERE c.code_digest=$1 AND c.redirect_uri=$2 AND g.client_id=$3 \
             AND c.consumed_at IS NULL AND c.expires_at>clock_timestamp() \
             AND g.revoked_at IS NULL AND a.email_verified_at IS NOT NULL AND p.enabled \
             FOR UPDATE OF c",
        ).bind(content_hash(code.as_bytes())).bind(redirect_uri).bind(client_id)
        .fetch_optional(&mut *tx).await?;
        let row = row.ok_or_else(|| ServiceError::forbidden("invalid authorization code"))?;
        let challenge: String = row.try_get("code_challenge")?;
        let expected = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        if challenge != expected {
            return Err(ServiceError::forbidden("invalid PKCE verifier"));
        }
        sqlx::query("UPDATE oauth_authorization_codes SET consumed_at=clock_timestamp() WHERE code_digest=$1")
            .bind(content_hash(code.as_bytes())).execute(&mut *tx).await?;
        let pair = issue_pair(
            &mut tx,
            row.try_get("grant_id")?,
            row.try_get("account_id")?,
            row.try_get("tenant_id")?,
            row.try_get("principal_id")?,
            row.try_get("client_name")?,
        )
        .await?;
        tx.commit().await?;
        Ok(pair)
    }

    pub async fn refresh_oauth(
        &self,
        refresh_token: &str,
        client_id: &str,
    ) -> ServiceResult<TokenPair> {
        let mut tx = self.pool().begin().await?;
        let row = sqlx::query(
            "SELECT r.grant_id,g.account_id,g.tenant_id,g.principal_id,cl.client_name \
             FROM oauth_refresh_tokens r JOIN oauth_grants g ON g.id=r.grant_id \
             JOIN oauth_clients cl ON cl.client_id=g.client_id \
             JOIN signup_accounts a ON a.id=g.account_id \
             JOIN principals p ON p.id=g.principal_id AND p.tenant_id=g.tenant_id \
             WHERE r.token_digest=$1 AND g.client_id=$2 AND r.consumed_at IS NULL \
             AND r.revoked_at IS NULL AND r.expires_at>clock_timestamp() \
             AND g.revoked_at IS NULL AND a.email_verified_at IS NOT NULL AND p.enabled \
             FOR UPDATE OF r",
        )
        .bind(content_hash(refresh_token.as_bytes()))
        .bind(client_id)
        .fetch_optional(&mut *tx)
        .await?;
        let row = row.ok_or_else(|| ServiceError::forbidden("invalid refresh token"))?;
        sqlx::query(
            "UPDATE oauth_refresh_tokens SET consumed_at=clock_timestamp() WHERE token_digest=$1",
        )
        .bind(content_hash(refresh_token.as_bytes()))
        .execute(&mut *tx)
        .await?;
        let pair = issue_pair(
            &mut tx,
            row.try_get("grant_id")?,
            row.try_get("account_id")?,
            row.try_get("tenant_id")?,
            row.try_get("principal_id")?,
            row.try_get("client_name")?,
        )
        .await?;
        tx.commit().await?;
        Ok(pair)
    }
}

async fn issue_pair(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    grant_id: Uuid,
    account_id: Uuid,
    tenant_id: Uuid,
    principal_id: Uuid,
    client_name: String,
) -> ServiceResult<TokenPair> {
    let access_token = fresh_token("oa_");
    let refresh_token = fresh_token("or_");
    let access_digest = content_hash(access_token.as_bytes());
    let access_expiry = Utc::now() + Duration::hours(1);
    sqlx::query("INSERT INTO principal_credentials(id,tenant_id,principal_id,credential_digest,valid_until) VALUES($1,$2,$3,$4,$5)")
        .bind(Uuid::new_v4()).bind(tenant_id).bind(principal_id).bind(&access_digest).bind(access_expiry)
        .execute(&mut **tx).await?;
    sqlx::query("INSERT INTO mcp_connections(id,tenant_id,principal_id,account_id,credential_digest,client_name,oauth_grant_id) VALUES($1,$2,$3,$4,$5,$6,$7)")
        .bind(Uuid::new_v4()).bind(tenant_id).bind(principal_id).bind(account_id).bind(access_digest)
        .bind(client_name).bind(grant_id).execute(&mut **tx).await?;
    sqlx::query(
        "INSERT INTO oauth_refresh_tokens(token_digest,grant_id,expires_at) VALUES($1,$2,$3)",
    )
    .bind(content_hash(refresh_token.as_bytes()))
    .bind(grant_id)
    .bind(Utc::now() + Duration::days(365))
    .execute(&mut **tx)
    .await?;
    Ok(TokenPair {
        access_token,
        refresh_token,
        expires_in: 3600,
    })
}
