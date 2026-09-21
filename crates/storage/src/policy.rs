//! Static SQL identity, ownership, authorization and attenuation boundary.
use crate::{Result, StorageError};
use chrono::{DateTime, Utc};
use ownstate_domain::*;
use sqlx::{Postgres, Transaction};
use std::str::FromStr;
use uuid::Uuid;

/// Identity materialized only by storage after credential, bootstrap, or
/// child-execution verification. Its fields and constructor are deliberately
/// private so interface adapters cannot forge a caller in Rust or JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedPrincipal {
    id: PrincipalId,
    tenant_id: TenantId,
    kind: PrincipalKind,
}

impl AuthenticatedPrincipal {
    fn from_verified_record(id: PrincipalId, tenant_id: TenantId, kind: PrincipalKind) -> Self {
        Self {
            id,
            tenant_id,
            kind,
        }
    }

    pub const fn id(&self) -> PrincipalId {
        self.id
    }

    pub const fn tenant_id(&self) -> TenantId {
        self.tenant_id
    }

    pub const fn kind(&self) -> PrincipalKind {
        self.kind
    }
}

pub struct PersonalPolicyBootstrap {
    pub principal: AuthenticatedPrincipal,
    pub workspace_id: WorkspaceId,
    pub grant_id: GrantId,
}
pub async fn bootstrap_personal_policy<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    tenant: TenantId,
) -> Result<PersonalPolicyBootstrap> {
    let row:(Uuid,Uuid,Uuid)=sqlx::query_as("WITH ids AS (SELECT ownstate_stable_uuid('legacy-user:'||$1::text)u,ownstate_stable_uuid('legacy-personal-workspace:'||$1::text)w,ownstate_stable_uuid('legacy-personal-principal:'||$1::text)p,ownstate_stable_uuid('legacy-personal-grant:'||$1::text)g),
 insu AS (INSERT INTO users(id,tenant_id,display_name) SELECT u,$1,'Personal owner' FROM ids ON CONFLICT DO NOTHING),
 insw AS (INSERT INTO workspaces(id,tenant_id,name,personal_user_id) SELECT w,$1,'Personal',u FROM ids ON CONFLICT DO NOTHING),
 insp AS (INSERT INTO principals(id,tenant_id,kind,subject_id) SELECT p,$1,'USER',u FROM ids ON CONFLICT DO NOTHING),
 insg AS (INSERT INTO authorization_grants(id,tenant_id,principal_id,workspace_id,actions,delegable_actions,classification_ceiling,allowed_destinations) SELECT g,$1,p,w,$2,$3,'SECRET',$4 FROM ids ON CONFLICT DO NOTHING)
 SELECT p,w,g FROM ids")
 .bind(tenant.as_uuid()).bind(PolicyAction::ALL.iter().map(|a|a.as_str()).collect::<Vec<_>>())
 .bind(vec!["READ_PROJECT","READ_EVIDENCE","READ_KNOWLEDGE","COMPILE_CONTEXT","CREATE_SESSION","APPEND_EVIDENCE","PROPOSE_KNOWLEDGE"])
 .bind(vec!["LOCAL:fastembed","LOCAL:deterministic"]).fetch_one(exec).await?;
    Ok(PersonalPolicyBootstrap {
        principal: AuthenticatedPrincipal::from_verified_record(
            row.0.into(),
            tenant,
            PrincipalKind::User,
        ),
        workspace_id: row.1.into(),
        grant_id: row.2.into(),
    })
}
#[derive(Debug, Clone)]
pub struct NewPrincipal {
    pub id: PrincipalId,
    pub tenant_id: TenantId,
    pub kind: PrincipalKind,
    pub subject_id: Uuid,
}
pub async fn insert_principal<'e>(exec: impl sqlx::PgExecutor<'e>, p: &NewPrincipal) -> Result<()> {
    sqlx::query("INSERT INTO principals(id,tenant_id,kind,subject_id)VALUES($1,$2,$3,$4)")
        .bind(p.id.as_uuid())
        .bind(p.tenant_id.as_uuid())
        .bind(p.kind.as_str())
        .bind(p.subject_id)
        .execute(exec)
        .await?;
    Ok(())
}
pub async fn register_credential<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    id: CredentialId,
    principal: &AuthenticatedPrincipal,
    digest: &str,
    valid_until: Option<DateTime<Utc>>,
) -> Result<()> {
    let row:Option<(Uuid,)>=sqlx::query_as("INSERT INTO principal_credentials(id,tenant_id,principal_id,credential_digest,valid_until)VALUES($1,$2,$3,$4,$5) ON CONFLICT(credential_digest) DO UPDATE SET credential_digest=EXCLUDED.credential_digest WHERE principal_credentials.tenant_id=EXCLUDED.tenant_id AND principal_credentials.principal_id=EXCLUDED.principal_id AND principal_credentials.valid_until IS NOT DISTINCT FROM EXCLUDED.valid_until RETURNING id").bind(id.as_uuid()).bind(principal.tenant_id().as_uuid()).bind(principal.id().as_uuid()).bind(digest).bind(valid_until).fetch_optional(exec).await?;
    if row.is_none() {
        return Err(StorageError::Conflict(
            "credential digest belongs to different identity".into(),
        ));
    }
    Ok(())
}
pub async fn resolve_credential<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    digest: &str,
    now: DateTime<Utc>,
) -> Result<Option<AuthenticatedPrincipal>> {
    let row: Option<(Uuid, Uuid, String)> = sqlx::query_as(
        "SELECT principal_id,tenant_id,principal_kind FROM ownstate_resolve_credential($1,$2)",
    )
    .bind(digest)
    .bind(now)
    .fetch_optional(exec)
    .await?;
    row.map(|(id, t, k)| -> Result<AuthenticatedPrincipal> {
        Ok(AuthenticatedPrincipal::from_verified_record(
            id.into(),
            t.into(),
            PrincipalKind::from_str(&k)?,
        ))
    })
    .transpose()
}
pub async fn set_principal_enabled<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    tenant: TenantId,
    id: PrincipalId,
    enabled: bool,
) -> Result<bool> {
    Ok(
        sqlx::query("UPDATE principals SET enabled=$3 WHERE tenant_id=$1 AND id=$2")
            .bind(tenant.as_uuid())
            .bind(id.as_uuid())
            .bind(enabled)
            .execute(exec)
            .await?
            .rows_affected()
            == 1,
    )
}
pub async fn revoke_credential<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    id: CredentialId,
    at: DateTime<Utc>,
) -> Result<bool> {
    Ok(sqlx::query(
        "UPDATE principal_credentials SET revoked_at=$2 WHERE id=$1 AND revoked_at IS NULL",
    )
    .bind(id.as_uuid())
    .bind(at)
    .execute(exec)
    .await?
    .rows_affected()
        == 1)
}

pub async fn resource_for_project<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    tenant: TenantId,
    project: ProjectId,
) -> Result<ResourceScope> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT workspace_id FROM project_ownership WHERE tenant_id=$1 AND project_id=$2",
    )
    .bind(tenant.as_uuid())
    .bind(project.as_uuid())
    .fetch_optional(exec)
    .await?;
    Ok(ResourceScope {
        tenant_id: tenant,
        workspace_id: row
            .ok_or_else(|| StorageError::not_found("project ownership"))?
            .0
            .into(),
        project_id: Some(project),
    })
}
pub async fn resource_for_session<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    tenant: TenantId,
    id: SessionId,
) -> Result<ResourceScope> {
    let row:Option<(Uuid,Uuid)>=sqlx::query_as("SELECT o.workspace_id,s.project_id FROM sessions s JOIN project_ownership o ON o.project_id=s.project_id AND o.tenant_id=s.tenant_id WHERE s.tenant_id=$1 AND s.id=$2").bind(tenant.as_uuid()).bind(id.as_uuid()).fetch_optional(exec).await?;
    let (w, p) = row.ok_or_else(|| StorageError::not_found("session ownership"))?;
    Ok(ResourceScope {
        tenant_id: tenant,
        workspace_id: w.into(),
        project_id: Some(p.into()),
    })
}
pub async fn resource_for_candidate<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    tenant: TenantId,
    id: CandidateId,
) -> Result<ResourceScope> {
    let row:Option<(Uuid,Uuid)>=sqlx::query_as("SELECT o.workspace_id,c.project_id FROM candidate_knowledge c JOIN project_ownership o ON o.project_id=c.project_id AND o.tenant_id=c.tenant_id WHERE c.tenant_id=$1 AND c.id=$2").bind(tenant.as_uuid()).bind(id.as_uuid()).fetch_optional(exec).await?;
    scope(tenant, row, "candidate ownership")
}
pub async fn resource_for_institutional_candidate<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    tenant: TenantId,
    id: GraphCandidateId,
) -> Result<ResourceScope> {
    let row:Option<(Uuid,Uuid)>=sqlx::query_as("SELECT o.workspace_id,c.project_id FROM institutional_candidates c JOIN project_ownership o ON o.project_id=c.project_id AND o.tenant_id=c.tenant_id WHERE c.tenant_id=$1 AND c.id=$2").bind(tenant.as_uuid()).bind(id.as_uuid()).fetch_optional(exec).await?;
    scope(tenant, row, "institutional candidate ownership")
}
pub async fn resource_for_knowledge_item<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    tenant: TenantId,
    id: KnowledgeItemId,
) -> Result<ResourceScope> {
    let row:Option<(Uuid,Uuid)>=sqlx::query_as("SELECT o.workspace_id,k.project_id FROM knowledge_items k JOIN project_ownership o ON o.project_id=k.project_id AND o.tenant_id=k.tenant_id WHERE k.tenant_id=$1 AND k.id=$2").bind(tenant.as_uuid()).bind(id.as_uuid()).fetch_optional(exec).await?;
    scope(tenant, row, "knowledge ownership")
}
pub async fn resource_for_knowledge_version<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    tenant: TenantId,
    id: KnowledgeVersionId,
) -> Result<ResourceScope> {
    let row:Option<(Uuid,Uuid)>=sqlx::query_as("SELECT o.workspace_id,k.project_id FROM knowledge_versions v JOIN knowledge_items k ON k.id=v.knowledge_item_id JOIN project_ownership o ON o.project_id=k.project_id AND o.tenant_id=k.tenant_id WHERE k.tenant_id=$1 AND v.id=$2").bind(tenant.as_uuid()).bind(id.as_uuid()).fetch_optional(exec).await?;
    scope(tenant, row, "knowledge version ownership")
}
fn scope(tenant: TenantId, row: Option<(Uuid, Uuid)>, name: &'static str) -> Result<ResourceScope> {
    let (w, p) = row.ok_or_else(|| StorageError::not_found(name))?;
    Ok(ResourceScope {
        tenant_id: tenant,
        workspace_id: w.into(),
        project_id: Some(p.into()),
    })
}

pub async fn authorize_and_record(
    tx: &mut Transaction<'_, Postgres>,
    principal: &AuthenticatedPrincipal,
    request: &PolicyRequest,
) -> Result<Option<AuthorizedScope>> {
    let destination = request
        .destination
        .as_ref()
        .map(ModelDestination::policy_key);
    let row:Option<(Uuid,String,Option<i64>,Option<i32>)>=sqlx::query_as("SELECT g.id,g.classification_ceiling,g.max_tokens,g.max_items FROM principals p JOIN authorization_grants g ON g.principal_id=p.id AND g.tenant_id=p.tenant_id WHERE p.id=$1 AND p.tenant_id=$2 AND p.enabled AND g.revoked_at IS NULL AND g.valid_from<=$3 AND(g.valid_until IS NULL OR g.valid_until>$3) AND g.workspace_id=$4 AND(g.project_id IS NULL OR g.project_id=$5) AND (($5::uuid IS NULL AND EXISTS(SELECT 1 FROM workspaces w WHERE w.tenant_id=$2 AND w.id=$4)) OR EXISTS(SELECT 1 FROM project_ownership o WHERE o.tenant_id=$2 AND o.workspace_id=$4 AND o.project_id=$5)) AND ($9::bigint IS NULL OR $9>0) AND ($10::int IS NULL OR $10>0) AND $6=ANY(g.actions) AND ownstate_classification_rank($7)<=ownstate_classification_rank(g.classification_ceiling) AND($6<>'MODEL_EGRESS' OR $8::text=ANY(g.allowed_destinations)) AND(g.max_tokens IS NULL OR $9::bigint IS NOT NULL AND $9<=g.max_tokens) AND(g.max_items IS NULL OR $10::int IS NOT NULL AND $10<=g.max_items) AND(p.kind<>'AGENT' OR $11::uuid IS NOT NULL AND EXISTS(SELECT 1 FROM agent_executions x WHERE x.id=$11 AND x.tenant_id=p.tenant_id AND x.principal_id=p.id AND x.status='ACTIVE' AND x.expires_at>$3)) ORDER BY(g.project_id IS NOT NULL)DESC,ownstate_classification_rank(g.classification_ceiling),g.id LIMIT 1 FOR SHARE OF p,g")
 .bind(principal.id().as_uuid()).bind(request.resource.tenant_id.as_uuid()).bind(request.now).bind(request.resource.workspace_id.as_uuid()).bind(request.resource.project_id.map(|v|v.as_uuid())).bind(request.action.as_str()).bind(request.classification.as_str()).bind(&destination).bind(request.budget.max_tokens).bind(request.budget.max_items).bind(request.execution_id.map(|v|v.as_uuid())).fetch_optional(&mut **tx).await?;
    let allowed = row.is_some() && principal.tenant_id() == request.resource.tenant_id;
    let grant = row.as_ref().map(|r| GrantId::from_uuid(r.0));
    sqlx::query("INSERT INTO policy_decisions(id,tenant_id,principal_id,action,workspace_id,project_id,execution_id,allowed,reason_code,classification,destination,requested_tokens,requested_items,grant_id)VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)")
 .bind(PolicyDecisionId::generate().as_uuid()).bind(request.resource.tenant_id.as_uuid()).bind(principal.id().as_uuid()).bind(request.action.as_str()).bind(request.resource.workspace_id.as_uuid()).bind(request.resource.project_id.map(|v|v.as_uuid())).bind(request.execution_id.map(|v|v.as_uuid())).bind(allowed).bind(if allowed{"GRANT_MATCH"}else{"DEFAULT_DENY"}).bind(request.classification.as_str()).bind(&destination).bind(request.budget.max_tokens).bind(request.budget.max_items).bind(grant.map(|g|g.as_uuid())).execute(&mut **tx).await?;
    row.map(|(id, class, tokens, items)| -> Result<AuthorizedScope> {
        Ok(AuthorizedScope::from_policy(
            id.into(),
            request.resource.clone(),
            class.parse()?,
            destination,
            ExecutionBudget {
                max_tokens: tokens,
                max_items: items,
            },
        ))
    })
    .transpose()
}

pub async fn insert_grant<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    id: GrantId,
    principal: &AuthenticatedPrincipal,
    spec: &GrantSpec,
    valid_from: DateTime<Utc>,
) -> Result<()> {
    spec.validate().map_err(StorageError::Conflict)?;
    sqlx::query("INSERT INTO authorization_grants(id,tenant_id,principal_id,workspace_id,project_id,actions,delegable_actions,classification_ceiling,allowed_destinations,max_tokens,max_items,valid_from,valid_until)VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)").bind(id.as_uuid()).bind(spec.tenant_id.as_uuid()).bind(principal.id().as_uuid()).bind(spec.workspace_id.as_uuid()).bind(spec.project_id.map(|v|v.as_uuid())).bind(spec.actions.iter().map(|v|v.as_str()).collect::<Vec<_>>()).bind(spec.delegable_actions.iter().map(|v|v.as_str()).collect::<Vec<_>>()).bind(spec.classification_ceiling.as_str()).bind(&spec.allowed_destinations).bind(spec.budget.max_tokens).bind(spec.budget.max_items).bind(valid_from).bind(spec.valid_until).execute(exec).await?;
    Ok(())
}
pub async fn revoke_grant<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    id: GrantId,
    at: DateTime<Utc>,
) -> Result<bool> {
    Ok(sqlx::query(
        "UPDATE authorization_grants SET revoked_at=$2 WHERE id=$1 AND revoked_at IS NULL",
    )
    .bind(id.as_uuid())
    .bind(at)
    .execute(exec)
    .await?
    .rows_affected()
        == 1)
}

pub struct ChildExecution {
    pub principal: AuthenticatedPrincipal,
    pub execution_id: AgentExecutionId,
    pub grant_id: GrantId,
}

pub struct NewChildExecution<'a> {
    pub grant: &'a GrantSpec,
    pub task: &'a str,
    pub harness: &'a str,
    pub destination: Option<&'a str>,
    pub expires_at: DateTime<Utc>,
}

type ParentGrantRow = (
    Uuid,
    Uuid,
    Option<Uuid>,
    Vec<String>,
    Vec<String>,
    String,
    Vec<String>,
    Option<i64>,
    Option<i32>,
    Option<DateTime<Utc>>,
);

pub async fn create_child_execution(
    tx: &mut Transaction<'_, Postgres>,
    parent: &AuthenticatedPrincipal,
    parent_grant: GrantId,
    request: NewChildExecution<'_>,
) -> Result<ChildExecution> {
    let child = request.grant;
    if child.actions.iter().any(|action| {
        matches!(
            action,
            PolicyAction::DelegateAgent | PolicyAction::AttributeHumanProposal
        )
    }) || child.delegable_actions.iter().any(|action| {
        matches!(
            action,
            PolicyAction::DelegateAgent | PolicyAction::AttributeHumanProposal
        )
    }) {
        return Err(StorageError::Conflict(
            "child executions cannot delegate or claim human attribution".into(),
        ));
    }
    let row:Option<ParentGrantRow>=sqlx::query_as("SELECT g.tenant_id,g.workspace_id,g.project_id,g.actions,g.delegable_actions,g.classification_ceiling,g.allowed_destinations,g.max_tokens,g.max_items,g.valid_until FROM authorization_grants g JOIN principals p ON p.id=g.principal_id AND p.tenant_id=g.tenant_id WHERE g.id=$1 AND p.id=$2 AND p.enabled AND g.revoked_at IS NULL AND g.valid_from<=clock_timestamp() AND(g.valid_until IS NULL OR g.valid_until>clock_timestamp()) AND 'DELEGATE_AGENT'=ANY(g.actions) FOR SHARE OF g,p").bind(parent_grant.as_uuid()).bind(parent.id().as_uuid()).fetch_optional(&mut **tx).await?;
    let (
        tenant,
        workspace,
        parent_project,
        _actions,
        delegable,
        class,
        destinations,
        tokens,
        items,
        valid_until,
    ) = row.ok_or_else(|| StorageError::Conflict("parent delegation unavailable".into()))?;
    let parent_spec = GrantSpec {
        tenant_id: tenant.into(),
        workspace_id: workspace.into(),
        project_id: parent_project.map(Into::into),
        actions: {
            let mut a = delegable
                .iter()
                .map(|v| PolicyAction::from_str(v))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            a.push(PolicyAction::DelegateAgent);
            a
        },
        delegable_actions: delegable
            .iter()
            .map(|v| PolicyAction::from_str(v))
            .collect::<std::result::Result<Vec<_>, _>>()?,
        classification_ceiling: class.parse()?,
        allowed_destinations: destinations,
        budget: ExecutionBudget {
            max_tokens: tokens,
            max_items: items,
        },
        valid_until,
    };
    child
        .attenuates(&parent_spec)
        .map_err(StorageError::Conflict)?;
    if request.expires_at <= Utc::now()
        || request
            .destination
            .is_some_and(|d| !child.allowed_destinations.iter().any(|v| v == d))
    {
        return Err(StorageError::Conflict(
            "invalid child execution expiry or destination".into(),
        ));
    }
    if child.valid_until.is_none_or(|v| v > request.expires_at) {
        return Err(StorageError::Conflict(
            "child grant exceeds execution expiry".into(),
        ));
    }
    let execution = AgentExecutionId::generate();
    let principal = AuthenticatedPrincipal::from_verified_record(
        PrincipalId::generate(),
        parent.tenant_id(),
        PrincipalKind::Agent,
    );
    let grant = GrantId::generate();
    insert_principal(
        &mut **tx,
        &NewPrincipal {
            id: principal.id(),
            tenant_id: principal.tenant_id(),
            kind: PrincipalKind::Agent,
            subject_id: execution.as_uuid(),
        },
    )
    .await?;
    insert_grant(&mut **tx, grant, &principal, child, Utc::now()).await?;
    sqlx::query("INSERT INTO agent_executions(id,tenant_id,principal_id,task,harness,model_destination,expires_at,status)VALUES($1,$2,$3,$4,$5,$6,$7,'ACTIVE')").bind(execution.as_uuid()).bind(principal.tenant_id().as_uuid()).bind(principal.id().as_uuid()).bind(request.task).bind(request.harness).bind(request.destination).bind(request.expires_at).execute(&mut **tx).await?;
    sqlx::query("INSERT INTO agent_grants(tenant_id,execution_id,grant_id,parent_grant_id)VALUES($1,$2,$3,$4)").bind(principal.tenant_id().as_uuid()).bind(execution.as_uuid()).bind(grant.as_uuid()).bind(parent_grant.as_uuid()).execute(&mut **tx).await?;
    Ok(ChildExecution {
        principal,
        execution_id: execution,
        grant_id: grant,
    })
}

pub async fn bind_project<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    tenant: TenantId,
    project: ProjectId,
    workspace: WorkspaceId,
    repository: Option<RepositoryId>,
    domain: OwnershipDomain,
) -> Result<()> {
    let row:Option<(Uuid,)>=sqlx::query_as("INSERT INTO project_ownership(tenant_id,project_id,workspace_id,repository_id,ownership_domain)VALUES($1,$2,$3,$4,$5) ON CONFLICT(project_id) DO UPDATE SET project_id=EXCLUDED.project_id WHERE project_ownership.tenant_id=EXCLUDED.tenant_id AND project_ownership.workspace_id=EXCLUDED.workspace_id AND project_ownership.repository_id IS NOT DISTINCT FROM EXCLUDED.repository_id AND project_ownership.ownership_domain=EXCLUDED.ownership_domain RETURNING project_id").bind(tenant.as_uuid()).bind(project.as_uuid()).bind(workspace.as_uuid()).bind(repository.map(|v|v.as_uuid())).bind(domain.as_str()).fetch_optional(exec).await?;
    if row.is_none() {
        return Err(StorageError::Conflict(
            "project already has different ownership".into(),
        ));
    }
    Ok(())
}

pub struct ServicePolicyBootstrap {
    pub principal: AuthenticatedPrincipal,
    pub grant_id: GrantId,
}
pub async fn bootstrap_service_policy<'e>(
    exec: impl sqlx::PgExecutor<'e>,
    tenant: TenantId,
    label: &str,
    spec: &GrantSpec,
) -> Result<ServicePolicyBootstrap> {
    spec.validate().map_err(StorageError::Conflict)?;
    if label.is_empty() || label.len() > 100 || label.trim() != label {
        return Err(StorageError::Conflict(
            "invalid service principal label".into(),
        ));
    }
    if spec.tenant_id != tenant {
        return Err(StorageError::Conflict(
            "service grant tenant mismatch".into(),
        ));
    }
    let row:Option<(Uuid,Uuid)>=sqlx::query_as("WITH ids AS(SELECT ownstate_stable_uuid('service-subject:'||$1::text||':'||$2)s,ownstate_stable_uuid('service-principal:'||$1::text||':'||$2)p,ownstate_stable_uuid('service-grant:'||$1::text||':'||$2)g),
 insp AS(INSERT INTO principals(id,tenant_id,kind,subject_id)SELECT p,$1,'SERVICE',s FROM ids ON CONFLICT(tenant_id,kind,subject_id)DO UPDATE SET subject_id=EXCLUDED.subject_id RETURNING id),
 insg AS(INSERT INTO authorization_grants(id,tenant_id,principal_id,workspace_id,project_id,actions,delegable_actions,classification_ceiling,allowed_destinations,max_tokens,max_items,valid_until)SELECT g,$1,p,$3,$4,$5,$6,$7,$8,$9,$10,$11 FROM ids ON CONFLICT(id)DO UPDATE SET id=EXCLUDED.id WHERE authorization_grants.tenant_id=EXCLUDED.tenant_id AND authorization_grants.principal_id=EXCLUDED.principal_id AND authorization_grants.workspace_id=EXCLUDED.workspace_id AND authorization_grants.project_id IS NOT DISTINCT FROM EXCLUDED.project_id AND authorization_grants.actions=EXCLUDED.actions AND authorization_grants.delegable_actions=EXCLUDED.delegable_actions AND authorization_grants.classification_ceiling=EXCLUDED.classification_ceiling AND authorization_grants.allowed_destinations=EXCLUDED.allowed_destinations AND authorization_grants.max_tokens IS NOT DISTINCT FROM EXCLUDED.max_tokens AND authorization_grants.max_items IS NOT DISTINCT FROM EXCLUDED.max_items AND authorization_grants.valid_until IS NOT DISTINCT FROM EXCLUDED.valid_until RETURNING id)
 SELECT p,g FROM ids WHERE EXISTS(SELECT 1 FROM insp)AND EXISTS(SELECT 1 FROM insg)")
 .bind(tenant.as_uuid()).bind(label).bind(spec.workspace_id.as_uuid()).bind(spec.project_id.map(|v|v.as_uuid())).bind(spec.actions.iter().map(|v|v.as_str()).collect::<Vec<_>>()).bind(spec.delegable_actions.iter().map(|v|v.as_str()).collect::<Vec<_>>()).bind(spec.classification_ceiling.as_str()).bind(&spec.allowed_destinations).bind(spec.budget.max_tokens).bind(spec.budget.max_items).bind(spec.valid_until).fetch_optional(exec).await?;
    let (p, g) = row.ok_or_else(|| {
        StorageError::Conflict("service bootstrap conflicts with existing policy".into())
    })?;
    Ok(ServicePolicyBootstrap {
        principal: AuthenticatedPrincipal::from_verified_record(
            p.into(),
            tenant,
            PrincipalKind::Service,
        ),
        grant_id: g.into(),
    })
}

/// Install transaction-local identity for RLS-enabled deployments. Pool reuse
/// cannot retain it beyond commit/rollback.
pub async fn set_local_policy_context(
    tx: &mut Transaction<'_, Postgres>,
    principal: &AuthenticatedPrincipal,
) -> Result<()> {
    sqlx::query("SELECT set_config('ownstate.tenant_id',$1,true),set_config('ownstate.principal_id',$2,true)").bind(principal.tenant_id().to_string()).bind(principal.id().to_string()).execute(&mut **tx).await?;
    Ok(())
}
