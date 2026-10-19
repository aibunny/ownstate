//! Account-owned organization creation and invitation acceptance.
use chrono::{Duration, Utc};
use ownstate_domain::content_hash;
use sqlx::Row;
use uuid::Uuid;

use crate::accounts::fresh_token;
use crate::{AppServices, ServiceError, ServiceResult};

pub struct WorkspaceChoice {
    pub workspace_id: Uuid,
    pub name: String,
    pub kind: String,
}

pub struct Invitation {
    pub token: String,
    pub email: String,
}

impl AppServices {
    pub async fn account_workspaces(
        &self,
        account_id: Uuid,
    ) -> ServiceResult<Vec<WorkspaceChoice>> {
        let rows = sqlx::query(
            "SELECT m.workspace_id,w.name,m.kind FROM account_tenant_memberships m \
             JOIN workspaces w ON w.id=m.workspace_id AND w.tenant_id=m.tenant_id \
             JOIN signup_accounts a ON a.id=m.account_id \
             JOIN principals p ON p.id=m.principal_id AND p.tenant_id=m.tenant_id \
             WHERE m.account_id=$1 AND a.email_verified_at IS NOT NULL AND p.enabled \
             AND (w.personal_user_id=m.user_id OR EXISTS (SELECT 1 FROM organization_memberships om \
                 WHERE om.tenant_id=m.tenant_id AND om.organization_id=w.organization_id AND om.user_id=m.user_id)) \
             ORDER BY CASE m.kind WHEN 'PERSONAL' THEN 0 ELSE 1 END,w.name",
        ).bind(account_id).fetch_all(self.pool()).await?;
        rows.into_iter()
            .map(|row| {
                Ok(WorkspaceChoice {
                    workspace_id: row.try_get("workspace_id")?,
                    name: row.try_get("name")?,
                    kind: row.try_get("kind")?,
                })
            })
            .collect()
    }

    pub async fn create_organization(&self, account_id: Uuid, name: &str) -> ServiceResult<Uuid> {
        let name = name.trim();
        if name.is_empty() || name.len() > 200 {
            return Err(ServiceError::validation(
                "organization name must be 1 to 200 characters",
            ));
        }
        let mut tx = self.pool().begin().await?;
        let email: Option<String> = sqlx::query_scalar(
            "SELECT email FROM signup_accounts WHERE id=$1 AND email_verified_at IS NOT NULL FOR SHARE",
        ).bind(account_id).fetch_optional(&mut *tx).await?;
        let email = email.ok_or_else(|| ServiceError::forbidden("verified account required"))?;
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let principal_id = Uuid::new_v4();
        let organization_id = Uuid::new_v4();
        let workspace_id = Uuid::new_v4();
        sqlx::query("INSERT INTO users(id,tenant_id,display_name) VALUES($1,$2,$3)")
            .bind(user_id)
            .bind(tenant_id)
            .bind(&email)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO organizations(id,tenant_id,name) VALUES($1,$2,$3)")
            .bind(organization_id)
            .bind(tenant_id)
            .bind(name)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO organization_memberships(tenant_id,organization_id,user_id,role) VALUES($1,$2,$3,'OWNER')")
            .bind(tenant_id).bind(organization_id).bind(user_id).execute(&mut *tx).await?;
        sqlx::query(
            "INSERT INTO workspaces(id,tenant_id,name,organization_id) VALUES($1,$2,$3,$4)",
        )
        .bind(workspace_id)
        .bind(tenant_id)
        .bind(name)
        .bind(organization_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO principals(id,tenant_id,kind,subject_id) VALUES($1,$2,'USER',$3)")
            .bind(principal_id)
            .bind(tenant_id)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        insert_member_grant(&mut tx, tenant_id, principal_id, workspace_id).await?;
        sqlx::query("INSERT INTO account_tenant_memberships(account_id,tenant_id,user_id,principal_id,workspace_id,kind) VALUES($1,$2,$3,$4,$5,'ORGANIZATION')")
            .bind(account_id).bind(tenant_id).bind(user_id).bind(principal_id).bind(workspace_id)
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(organization_id)
    }

    pub async fn invite_to_organization(
        &self,
        account_id: Uuid,
        workspace_id: Uuid,
        email: &str,
    ) -> ServiceResult<Invitation> {
        let email = email.trim().to_ascii_lowercase();
        if email.len() > 320 || !email.contains('@') {
            return Err(ServiceError::validation("invalid email"));
        }
        let mut tx = self.pool().begin().await?;
        let org: Option<(Uuid, Uuid)> = sqlx::query_as(
            "SELECT m.tenant_id,w.organization_id FROM account_tenant_memberships m \
             JOIN workspaces w ON w.id=m.workspace_id AND w.tenant_id=m.tenant_id \
             JOIN organization_memberships om ON om.tenant_id=m.tenant_id \
             AND om.organization_id=w.organization_id AND om.user_id=m.user_id \
             WHERE m.account_id=$1 AND m.workspace_id=$2 AND m.kind='ORGANIZATION' \
             AND om.role IN ('OWNER','ADMIN') FOR SHARE OF m,om",
        )
        .bind(account_id)
        .bind(workspace_id)
        .fetch_optional(&mut *tx)
        .await?;
        let (tenant_id, organization_id) =
            org.ok_or_else(|| ServiceError::forbidden("organization admin required"))?;
        let token = fresh_token("invite_");
        sqlx::query("INSERT INTO organization_invitations(token_digest,tenant_id,organization_id,workspace_id,invited_email,invited_by_account_id,expires_at) VALUES($1,$2,$3,$4,$5,$6,$7)")
            .bind(content_hash(token.as_bytes())).bind(tenant_id).bind(organization_id).bind(workspace_id)
            .bind(&email).bind(account_id).bind(Utc::now()+Duration::days(7))
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(Invitation { token, email })
    }

    pub async fn accept_organization_invitation(
        &self,
        account_id: Uuid,
        token: &str,
    ) -> ServiceResult<()> {
        let mut tx = self.pool().begin().await?;
        let row = sqlx::query(
            "SELECT i.tenant_id,i.organization_id,i.workspace_id,a.email \
             FROM organization_invitations i JOIN signup_accounts a ON a.id=$2 \
             WHERE i.token_digest=$1 AND i.accepted_at IS NULL AND i.expires_at>clock_timestamp() \
             AND a.email_verified_at IS NOT NULL AND i.invited_email=a.email FOR UPDATE OF i",
        )
        .bind(content_hash(token.as_bytes()))
        .bind(account_id)
        .fetch_optional(&mut *tx)
        .await?;
        let row = row.ok_or_else(|| ServiceError::forbidden("invitation is invalid or expired"))?;
        let tenant_id: Uuid = row.try_get("tenant_id")?;
        let organization_id: Uuid = row.try_get("organization_id")?;
        let workspace_id: Uuid = row.try_get("workspace_id")?;
        let email: String = row.try_get("email")?;
        let user_id = Uuid::new_v4();
        let principal_id = Uuid::new_v4();
        sqlx::query("INSERT INTO users(id,tenant_id,display_name) VALUES($1,$2,$3)")
            .bind(user_id)
            .bind(tenant_id)
            .bind(email)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO organization_memberships(tenant_id,organization_id,user_id,role) VALUES($1,$2,$3,'MEMBER')")
            .bind(tenant_id).bind(organization_id).bind(user_id).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO principals(id,tenant_id,kind,subject_id) VALUES($1,$2,'USER',$3)")
            .bind(principal_id)
            .bind(tenant_id)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        insert_member_grant(&mut tx, tenant_id, principal_id, workspace_id).await?;
        sqlx::query("INSERT INTO account_tenant_memberships(account_id,tenant_id,user_id,principal_id,workspace_id,kind) VALUES($1,$2,$3,$4,$5,'ORGANIZATION')")
            .bind(account_id).bind(tenant_id).bind(user_id).bind(principal_id).bind(workspace_id)
            .execute(&mut *tx).await?;
        sqlx::query("UPDATE organization_invitations SET accepted_at=clock_timestamp() WHERE token_digest=$1")
            .bind(content_hash(token.as_bytes())).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
}

async fn insert_member_grant(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant: Uuid,
    principal: Uuid,
    workspace: Uuid,
) -> ServiceResult<()> {
    sqlx::query("INSERT INTO authorization_grants(id,tenant_id,principal_id,workspace_id,actions,classification_ceiling,allowed_destinations) VALUES($1,$2,$3,$4,$5,'CONFIDENTIAL',$6)")
        .bind(Uuid::new_v4()).bind(tenant).bind(principal).bind(workspace)
        .bind(vec!["READ_PROJECT","READ_EVIDENCE","READ_KNOWLEDGE","COMPILE_CONTEXT","CREATE_PROJECT","CREATE_SESSION","APPEND_EVIDENCE","PROPOSE_KNOWLEDGE","GENERATE_EMBEDDING","MODEL_EGRESS"])
        .bind(vec!["LOCAL:fastembed","LOCAL:deterministic"])
        .execute(&mut **tx).await?;
    Ok(())
}
