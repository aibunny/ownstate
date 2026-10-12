//! Human account and account-owned MCP connection operations. These are never
//! exposed as model-facing tools.
use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use chrono::{Duration, Utc};
use ownstate_domain::content_hash;
use rand_core::OsRng;
use sqlx::Row;
use uuid::Uuid;

use crate::{AppServices, ServiceError, ServiceResult};

pub struct PendingSignup {
    pub verification_token: String,
    pub email: String,
}

pub struct LoginSession {
    pub token: String,
    pub account_id: Uuid,
}

pub struct NewMcpConnection {
    pub token: String,
    pub connection_id: Uuid,
}

pub struct ConnectionSummary {
    pub id: Uuid,
    pub client_name: String,
    pub workspace_name: String,
    pub oauth_grant_id: Option<Uuid>,
    pub created_at: chrono::DateTime<Utc>,
}

pub struct AccountConversationSummary {
    pub id: Uuid,
    pub source: String,
    pub title: Option<String>,
    pub space: String,
    pub created_at: chrono::DateTime<Utc>,
}

pub struct AccountConversationEvent {
    pub actor_type: String,
    pub content: Option<String>,
    pub occurred_at: chrono::DateTime<Utc>,
}

pub(crate) fn fresh_token(prefix: &str) -> String {
    format!(
        "{prefix}{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    )
}

impl AppServices {
    pub(crate) async fn check_auth_rate_limit(
        &self,
        action: &str,
        subject: &str,
        limit: i32,
        window_seconds: i32,
    ) -> ServiceResult<()> {
        let digest =
            content_hash(format!("{action}:{}", subject.trim().to_ascii_lowercase()).as_bytes());
        let attempts: i32 = sqlx::query_scalar(
            "INSERT INTO auth_rate_limits(key_digest,window_start,attempts) VALUES($1,clock_timestamp(),1) \
             ON CONFLICT(key_digest) DO UPDATE SET \
             attempts=CASE WHEN auth_rate_limits.window_start < clock_timestamp()-($2::int * interval '1 second') \
                 THEN 1 ELSE LEAST(auth_rate_limits.attempts+1,$3::int+1) END, \
             window_start=CASE WHEN auth_rate_limits.window_start < clock_timestamp()-($2::int * interval '1 second') \
                 THEN clock_timestamp() ELSE auth_rate_limits.window_start END, \
             updated_at=clock_timestamp() RETURNING attempts")
            .bind(digest).bind(window_seconds).bind(limit).fetch_one(self.pool()).await?;
        if attempts > limit {
            return Err(ServiceError::forbidden(
                "too many attempts; try again later",
            ));
        }
        Ok(())
    }
    pub async fn account_conversations(
        &self,
        account_id: Uuid,
    ) -> ServiceResult<Vec<AccountConversationSummary>> {
        let rows = sqlx::query(
            "SELECT t.id,t.source,t.title,w.name AS space,t.created_at \
             FROM conversation_threads t JOIN account_tenant_memberships m \
             ON m.tenant_id=t.tenant_id AND m.workspace_id=t.workspace_id AND m.principal_id=t.owner_principal_id \
             JOIN workspaces w ON w.tenant_id=m.tenant_id AND w.id=m.workspace_id \
             JOIN principals p ON p.tenant_id=m.tenant_id AND p.id=m.principal_id \
             JOIN authorization_grants g ON g.tenant_id=m.tenant_id AND g.principal_id=m.principal_id \
                  AND g.workspace_id=m.workspace_id AND g.project_id IS NULL \
             WHERE m.account_id=$1 AND p.enabled \
             AND g.revoked_at IS NULL AND g.valid_from<=clock_timestamp() \
             AND (g.valid_until IS NULL OR g.valid_until>clock_timestamp()) AND 'READ_EVIDENCE'=ANY(g.actions) \
             AND (w.personal_user_id=m.user_id OR EXISTS (SELECT 1 FROM organization_memberships om \
                  WHERE om.tenant_id=m.tenant_id AND om.organization_id=w.organization_id AND om.user_id=m.user_id)) \
             ORDER BY t.created_at DESC LIMIT 100")
            .bind(account_id).fetch_all(self.pool()).await?;
        rows.into_iter()
            .map(|row| {
                Ok(AccountConversationSummary {
                    id: row.try_get("id")?,
                    source: row.try_get("source")?,
                    title: row.try_get("title")?,
                    space: row.try_get("space")?,
                    created_at: row.try_get("created_at")?,
                })
            })
            .collect()
    }

    pub async fn account_conversation_events(
        &self,
        account_id: Uuid,
        thread_id: Uuid,
    ) -> ServiceResult<Vec<AccountConversationEvent>> {
        let allowed =
            ownstate_domain::SecurityClassification::allowed_up_to(self.max_classification());
        let rows = sqlx::query(
            "SELECT e.actor_type,e.content,e.occurred_at FROM conversation_events e \
             JOIN conversation_threads t ON t.tenant_id=e.tenant_id AND t.id=e.thread_id \
             JOIN account_tenant_memberships m ON m.tenant_id=t.tenant_id \
                  AND m.workspace_id=t.workspace_id AND m.principal_id=t.owner_principal_id \
             JOIN workspaces w ON w.tenant_id=m.tenant_id AND w.id=m.workspace_id \
             JOIN principals p ON p.tenant_id=m.tenant_id AND p.id=m.principal_id \
             JOIN authorization_grants g ON g.tenant_id=m.tenant_id AND g.principal_id=m.principal_id \
                  AND g.workspace_id=m.workspace_id AND g.project_id IS NULL \
             WHERE m.account_id=$1 AND t.id=$2 AND p.enabled AND e.security_classification=ANY($3) \
             AND g.revoked_at IS NULL AND g.valid_from<=clock_timestamp() \
             AND (g.valid_until IS NULL OR g.valid_until>clock_timestamp()) AND 'READ_EVIDENCE'=ANY(g.actions) \
             AND array_position(ARRAY['PUBLIC','INTERNAL','CONFIDENTIAL','RESTRICTED','SECRET'],e.security_classification) \
                 <= array_position(ARRAY['PUBLIC','INTERNAL','CONFIDENTIAL','RESTRICTED','SECRET'],g.classification_ceiling) \
             AND (w.personal_user_id=m.user_id OR EXISTS (SELECT 1 FROM organization_memberships om \
                  WHERE om.tenant_id=m.tenant_id AND om.organization_id=w.organization_id AND om.user_id=m.user_id)) \
             ORDER BY e.sequence LIMIT 1000")
            .bind(account_id).bind(thread_id).bind(allowed).fetch_all(self.pool()).await?;
        rows.into_iter()
            .map(|row| {
                Ok(AccountConversationEvent {
                    actor_type: row.try_get("actor_type")?,
                    content: row.try_get("content")?,
                    occurred_at: row.try_get("occurred_at")?,
                })
            })
            .collect()
    }
    pub async fn resend_verification(&self, email: &str) -> ServiceResult<Option<PendingSignup>> {
        let email = email.trim().to_ascii_lowercase();
        self.check_auth_rate_limit("resend", &email, 4, 3600)
            .await?;
        let mut tx = self.pool().begin().await?;
        let account_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM signup_accounts WHERE email=$1 AND email_verified_at IS NULL FOR UPDATE")
            .bind(&email).fetch_optional(&mut *tx).await?;
        let Some(account_id) = account_id else {
            return Ok(None);
        };
        let recent: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM email_verification_challenges WHERE account_id=$1 AND created_at>clock_timestamp()-interval '1 hour'")
            .bind(account_id).fetch_one(&mut *tx).await?;
        if recent >= 4 {
            return Ok(None);
        }
        let token = fresh_token("verify_");
        sqlx::query("INSERT INTO email_verification_challenges(token_digest,account_id,expires_at) VALUES($1,$2,$3)")
            .bind(content_hash(token.as_bytes())).bind(account_id).bind(Utc::now()+Duration::hours(24))
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(Some(PendingSignup {
            verification_token: token,
            email,
        }))
    }
    pub async fn account_connections(
        &self,
        account_id: Uuid,
    ) -> ServiceResult<Vec<ConnectionSummary>> {
        let rows = sqlx::query(
            "SELECT DISTINCT ON (COALESCE(c.oauth_grant_id,c.id)) c.id,c.client_name,w.name AS workspace_name,c.oauth_grant_id,c.created_at \
             FROM mcp_connections c JOIN account_tenant_memberships m \
             ON m.account_id=c.account_id AND m.tenant_id=c.tenant_id AND m.principal_id=c.principal_id \
             JOIN workspaces w ON w.id=m.workspace_id AND w.tenant_id=m.tenant_id \
             LEFT JOIN oauth_grants g ON g.id=c.oauth_grant_id \
             WHERE c.account_id=$1 AND c.revoked_at IS NULL \
             AND (c.oauth_grant_id IS NULL OR g.revoked_at IS NULL) \
             ORDER BY COALESCE(c.oauth_grant_id,c.id),c.created_at DESC")
            .bind(account_id).fetch_all(self.pool()).await?;
        rows.into_iter()
            .map(|row| {
                Ok(ConnectionSummary {
                    id: row.try_get("id")?,
                    client_name: row.try_get("client_name")?,
                    workspace_name: row.try_get("workspace_name")?,
                    oauth_grant_id: row.try_get("oauth_grant_id")?,
                    created_at: row.try_get("created_at")?,
                })
            })
            .collect()
    }

    pub async fn revoke_account_connection(
        &self,
        account_id: Uuid,
        connection_id: Uuid,
    ) -> ServiceResult<bool> {
        let mut tx = self.pool().begin().await?;
        let grant_id: Option<Option<Uuid>> = sqlx::query_scalar(
            "SELECT oauth_grant_id FROM mcp_connections WHERE id=$1 AND account_id=$2 FOR UPDATE",
        )
        .bind(connection_id)
        .bind(account_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(grant_id) = grant_id else {
            return Ok(false);
        };
        if let Some(grant_id) = grant_id {
            sqlx::query("UPDATE oauth_grants SET revoked_at=clock_timestamp() WHERE id=$1 AND account_id=$2 AND revoked_at IS NULL")
                .bind(grant_id).bind(account_id).execute(&mut *tx).await?;
            sqlx::query("UPDATE oauth_refresh_tokens SET revoked_at=clock_timestamp() WHERE grant_id=$1 AND revoked_at IS NULL")
                .bind(grant_id).execute(&mut *tx).await?;
            let digests: Vec<String> = sqlx::query_scalar("UPDATE mcp_connections SET revoked_at=clock_timestamp() WHERE oauth_grant_id=$1 AND revoked_at IS NULL RETURNING credential_digest")
                .bind(grant_id).fetch_all(&mut *tx).await?;
            for digest in digests {
                sqlx::query("UPDATE principal_credentials SET revoked_at=clock_timestamp() WHERE credential_digest=$1 AND revoked_at IS NULL")
                    .bind(digest).execute(&mut *tx).await?;
            }
        } else {
            let digest: Option<String> = sqlx::query_scalar("UPDATE mcp_connections SET revoked_at=clock_timestamp() WHERE id=$1 AND account_id=$2 AND revoked_at IS NULL RETURNING credential_digest")
                .bind(connection_id).bind(account_id).fetch_optional(&mut *tx).await?;
            if let Some(digest) = digest {
                sqlx::query("UPDATE principal_credentials SET revoked_at=clock_timestamp() WHERE credential_digest=$1 AND revoked_at IS NULL")
                    .bind(digest).execute(&mut *tx).await?;
            }
        }
        tx.commit().await?;
        Ok(true)
    }
    pub async fn signup(&self, email: &str, password: &str) -> ServiceResult<PendingSignup> {
        let email = email.trim().to_ascii_lowercase();
        self.check_auth_rate_limit("signup", &email, 4, 3600)
            .await?;
        if email.len() > 320
            || email.len() < 3
            || !email.contains('@')
            || email.contains(char::is_whitespace)
        {
            return Err(ServiceError::validation("invalid email address"));
        }
        if password.len() < 12 || password.len() > 1024 {
            return Err(ServiceError::validation(
                "password must be 12 to 1024 characters",
            ));
        }
        let salt = SaltString::generate(&mut OsRng);
        let password_hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(ServiceError::internal)?
            .to_string();
        let account_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let workspace_id = Uuid::new_v4();
        let principal_id = Uuid::new_v4();
        let grant_id = Uuid::new_v4();
        let token = fresh_token("verify_");
        let mut tx = self.pool().begin().await?;
        sqlx::query("INSERT INTO signup_accounts(id,email,password_hash) VALUES($1,$2,$3)")
            .bind(account_id)
            .bind(&email)
            .bind(password_hash)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                if matches!(&e, sqlx::Error::Database(db) if db.is_unique_violation()) {
                    ServiceError::Conflict("account already exists".into())
                } else {
                    ServiceError::from(e)
                }
            })?;
        sqlx::query("INSERT INTO users(id,tenant_id,display_name) VALUES($1,$2,$3)")
            .bind(user_id)
            .bind(tenant_id)
            .bind(&email)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO workspaces(id,tenant_id,name,personal_user_id) VALUES($1,$2,'Personal',$3)")
            .bind(workspace_id).bind(tenant_id).bind(user_id).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO principals(id,tenant_id,kind,subject_id) VALUES($1,$2,'USER',$3)")
            .bind(principal_id)
            .bind(tenant_id)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO authorization_grants(id,tenant_id,principal_id,workspace_id,actions,classification_ceiling,allowed_destinations) VALUES($1,$2,$3,$4,$5,'CONFIDENTIAL',$6)")
            .bind(grant_id).bind(tenant_id).bind(principal_id).bind(workspace_id)
            .bind(vec!["READ_PROJECT","READ_EVIDENCE","READ_KNOWLEDGE","COMPILE_CONTEXT","CREATE_PROJECT","CREATE_SESSION","APPEND_EVIDENCE","PROPOSE_KNOWLEDGE"])
            .bind(vec!["LOCAL:fastembed","LOCAL:deterministic"])
            .execute(&mut *tx).await?;
        sqlx::query("INSERT INTO account_tenant_memberships(account_id,tenant_id,user_id,principal_id,workspace_id,kind) VALUES($1,$2,$3,$4,$5,'PERSONAL')")
            .bind(account_id).bind(tenant_id).bind(user_id).bind(principal_id).bind(workspace_id)
            .execute(&mut *tx).await?;
        sqlx::query("INSERT INTO email_verification_challenges(token_digest,account_id,expires_at) VALUES($1,$2,$3)")
            .bind(content_hash(token.as_bytes())).bind(account_id).bind(Utc::now() + Duration::hours(24))
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(PendingSignup {
            verification_token: token,
            email,
        })
    }

    pub async fn verify_signup(&self, token: &str) -> ServiceResult<()> {
        let digest = content_hash(token.as_bytes());
        let mut tx = self.pool().begin().await?;
        let account_id: Option<Uuid> = sqlx::query_scalar(
            "UPDATE email_verification_challenges SET consumed_at=clock_timestamp() \
             WHERE token_digest=$1 AND consumed_at IS NULL AND expires_at>clock_timestamp() \
             RETURNING account_id",
        )
        .bind(digest)
        .fetch_optional(&mut *tx)
        .await?;
        let account_id = account_id.ok_or(ServiceError::validation(
            "verification link is invalid or expired",
        ))?;
        sqlx::query("UPDATE signup_accounts SET email_verified_at=clock_timestamp() WHERE id=$1 AND email_verified_at IS NULL")
            .bind(account_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn login(&self, email: &str, password: &str) -> ServiceResult<LoginSession> {
        let email = email.trim().to_ascii_lowercase();
        self.check_auth_rate_limit("login", &email, 10, 900).await?;
        let row = sqlx::query(
            "SELECT id,password_hash,email_verified_at FROM signup_accounts WHERE email=$1",
        )
        .bind(email)
        .fetch_optional(self.pool())
        .await?;
        let row = row.ok_or_else(|| ServiceError::forbidden("invalid credentials"))?;
        let hash: String = row.try_get("password_hash")?;
        let parsed = PasswordHash::new(&hash).map_err(ServiceError::internal)?;
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .map_err(|_| ServiceError::forbidden("invalid credentials"))?;
        let verified: Option<chrono::DateTime<Utc>> = row.try_get("email_verified_at")?;
        if verified.is_none() {
            return Err(ServiceError::forbidden("email is not verified"));
        }
        let account_id: Uuid = row.try_get("id")?;
        let token = fresh_token("web_");
        sqlx::query(
            "INSERT INTO web_login_sessions(token_digest,account_id,expires_at) VALUES($1,$2,$3)",
        )
        .bind(content_hash(token.as_bytes()))
        .bind(account_id)
        .bind(Utc::now() + Duration::days(30))
        .execute(self.pool())
        .await?;
        Ok(LoginSession { token, account_id })
    }

    pub async fn account_for_web_session(&self, token: &str) -> ServiceResult<Uuid> {
        let account: Option<Uuid> = sqlx::query_scalar(
            "SELECT account_id FROM web_login_sessions WHERE token_digest=$1 \
             AND revoked_at IS NULL AND expires_at>clock_timestamp()",
        )
        .bind(content_hash(token.as_bytes()))
        .fetch_optional(self.pool())
        .await?;
        account.ok_or_else(|| ServiceError::forbidden("session is invalid or expired"))
    }

    pub async fn logout(&self, token: &str) -> ServiceResult<()> {
        sqlx::query("UPDATE web_login_sessions SET revoked_at=clock_timestamp() WHERE token_digest=$1 AND revoked_at IS NULL")
            .bind(content_hash(token.as_bytes())).execute(self.pool()).await?;
        Ok(())
    }

    pub async fn create_mcp_connection(
        &self,
        account_id: Uuid,
        workspace_id: Uuid,
        client_name: &str,
    ) -> ServiceResult<NewMcpConnection> {
        let client_name = client_name.trim();
        if client_name.is_empty() || client_name.len() > 100 {
            return Err(ServiceError::validation(
                "client name must be 1 to 100 characters",
            ));
        }
        let mut tx = self.pool().begin().await?;
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
            owner.ok_or_else(|| ServiceError::forbidden("verified personal account required"))?;
        let token = fresh_token("ownstate_mcp_");
        let digest = content_hash(token.as_bytes());
        let connection_id = Uuid::new_v4();
        sqlx::query("INSERT INTO principal_credentials(id,tenant_id,principal_id,credential_digest) VALUES($1,$2,$3,$4)")
            .bind(Uuid::new_v4()).bind(tenant_id).bind(principal_id).bind(&digest)
            .execute(&mut *tx).await?;
        sqlx::query("INSERT INTO mcp_connections(id,tenant_id,principal_id,account_id,credential_digest,client_name) VALUES($1,$2,$3,$4,$5,$6)")
            .bind(connection_id).bind(tenant_id).bind(principal_id).bind(account_id).bind(digest).bind(client_name)
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(NewMcpConnection {
            token,
            connection_id,
        })
    }

    pub async fn revoke_mcp_connection(
        &self,
        account_id: Uuid,
        connection_id: Uuid,
    ) -> ServiceResult<bool> {
        let mut tx = self.pool().begin().await?;
        let digest: Option<String> = sqlx::query_scalar(
            "UPDATE mcp_connections SET revoked_at=clock_timestamp() \
             WHERE id=$1 AND account_id=$2 AND revoked_at IS NULL RETURNING credential_digest",
        )
        .bind(connection_id)
        .bind(account_id)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(digest) = digest {
            sqlx::query("UPDATE principal_credentials SET revoked_at=clock_timestamp() WHERE credential_digest=$1 AND revoked_at IS NULL")
                .bind(digest).execute(&mut *tx).await?;
            tx.commit().await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
