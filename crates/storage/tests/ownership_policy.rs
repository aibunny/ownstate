//! Real PostgreSQL authorization and legacy-preservation acceptance.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use chrono::{Duration, Utc};
use ownstate_domain::*;
use ownstate_storage::policy::AuthenticatedPrincipal;
use ownstate_storage::test_support::{TestDb, fresh_db};
use ownstate_storage::{MIGRATOR, events, policy, projects, sessions};
use serde_json::json;
use sqlx::Acquire;
use sqlx::postgres::PgPoolOptions;
use std::sync::atomic::{AtomicU64, Ordering};
static LABEL: AtomicU64 = AtomicU64::new(1);
fn project(t: TenantId, name: &str, domain: OwnershipDomain) -> Project {
    Project {
        id: ProjectId::generate(),
        tenant_id: t,
        ownership_domain: domain,
        name: name.into(),
        description: None,
        metadata: json!({}),
        created_at: Utc::now(),
    }
}
fn session(p: &Project) -> Session {
    Session {
        id: SessionId::generate(),
        tenant_id: p.tenant_id,
        project_id: p.id,
        source: "legacy".into(),
        source_session_id: None,
        agent: None,
        provider: None,
        model: None,
        status: SessionStatus::Active,
        started_at: Utc::now(),
        ended_at: None,
        repository: None,
        initial_commit: None,
        final_commit: None,
        metadata: json!({}),
        created_at: Utc::now(),
    }
}
fn event(s: &Session) -> InteractionEvent {
    InteractionEvent {
        id: EventId::generate(),
        tenant_id: s.tenant_id,
        project_id: s.project_id,
        session_id: s.id,
        source: "legacy".into(),
        source_event_id: None,
        event_type: InteractionEventType::UserMessage,
        actor_type: ActorType::User,
        actor_id: None,
        sequence: 0,
        occurred_at: Utc::now(),
        model_provider: None,
        model_name: None,
        content: Some("preserved evidence".into()),
        content_hash: Some(content_hash(b"preserved evidence")),
        tool_name: None,
        tool_call_id: None,
        repository: None,
        branch: None,
        commit_sha: None,
        file_path: None,
        security_classification: SecurityClassification::Internal,
        metadata: json!({}),
        created_at: Utc::now(),
    }
}
async fn legacy_db() -> (TestDb, Project, Project, InteractionEvent) {
    let base = std::env::var("OWNSTATE_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://ownstate:ownstate_dev@127.0.0.1:5432/ownstate".into());
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(&base)
        .await
        .unwrap();
    let n = LABEL.fetch_add(1, Ordering::Relaxed);
    let name = format!(
        "ownstate_test_{}_{}_{}",
        Utc::now().timestamp(),
        std::process::id(),
        n
    );
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE {name}")))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
    let (prefix, _) = base.rsplit_once('/').unwrap();
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&format!("{prefix}/{name}"))
        .await
        .unwrap();
    let first = sqlx::migrate::Migrator::with_migrations(
        MIGRATOR
            .iter()
            .filter(|m| m.version <= 9)
            .cloned()
            .collect(),
    );
    first.run(&pool).await.unwrap();
    let t = TenantId::generate();
    let personal = project(t, "Personal legacy", OwnershipDomain::Personal);
    let org = project(t, "Org legacy", OwnershipDomain::Organization);
    projects::insert(&pool, &personal).await.unwrap();
    projects::insert(&pool, &org).await.unwrap();
    let s = session(&personal);
    sessions::insert(&pool, &s).await.unwrap();
    let e = event(&s);
    events::insert(&pool, &e).await.unwrap();
    MIGRATOR.run(&pool).await.unwrap();
    (TestDb { pool, name }, personal, org, e)
}
fn request(resource: ResourceScope, action: PolicyAction) -> PolicyRequest {
    PolicyRequest {
        action,
        resource,
        classification: SecurityClassification::Internal,
        destination: None,
        budget: ExecutionBudget::default(),
        now: Utc::now(),
        execution_id: None,
    }
}
async fn decide(
    db: &TestDb,
    p: &AuthenticatedPrincipal,
    r: &PolicyRequest,
) -> Option<AuthorizedScope> {
    let mut tx = db.pool.begin().await.unwrap();
    let d = policy::authorize_and_record(&mut tx, p, r).await.unwrap();
    tx.commit().await.unwrap();
    d
}
#[tokio::test]
async fn migration_preserves_history_bootstraps_personal_and_denies_organization() {
    let (db, personal, org, e) = legacy_db().await;
    assert_eq!(
        events::list_for_session(&db.pool, e.session_id)
            .await
            .unwrap()[0]
            .id,
        e.id
    );
    assert_eq!(
        projects::get(&db.pool, personal.tenant_id, personal.id)
            .await
            .unwrap()
            .id,
        personal.id
    );
    let bootstrap = policy::bootstrap_personal_policy(&db.pool, personal.tenant_id)
        .await
        .unwrap();
    let pr = policy::resource_for_project(&db.pool, personal.tenant_id, personal.id)
        .await
        .unwrap();
    assert!(
        decide(
            &db,
            &bootstrap.principal,
            &request(pr, PolicyAction::ReadKnowledge)
        )
        .await
        .is_some()
    );
    let or = policy::resource_for_project(&db.pool, org.tenant_id, org.id)
        .await
        .unwrap();
    assert!(
        decide(
            &db,
            &bootstrap.principal,
            &request(or, PolicyAction::ReadKnowledge)
        )
        .await
        .is_none()
    );
    let (count,): (i64,) = sqlx::query_as("SELECT count(*) FROM organization_memberships")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 0)
}
#[tokio::test]
async fn credentials_are_server_resolved_disabled_expired_and_revoked() {
    let (db, p, _, _) = legacy_db().await;
    let b = policy::bootstrap_personal_policy(&db.pool, p.tenant_id)
        .await
        .unwrap();
    let id = CredentialId::generate();
    policy::register_credential(&db.pool, id, &b.principal, "blake3:fictional", None)
        .await
        .unwrap();
    assert_eq!(
        policy::resolve_credential(&db.pool, "blake3:fictional", Utc::now())
            .await
            .unwrap()
            .unwrap()
            .id(),
        b.principal.id()
    );
    policy::set_principal_enabled(&db.pool, p.tenant_id, b.principal.id(), false)
        .await
        .unwrap();
    assert!(
        policy::resolve_credential(&db.pool, "blake3:fictional", Utc::now())
            .await
            .unwrap()
            .is_none()
    );
    policy::set_principal_enabled(&db.pool, p.tenant_id, b.principal.id(), true)
        .await
        .unwrap();
    policy::revoke_credential(&db.pool, id, Utc::now())
        .await
        .unwrap();
    assert!(
        policy::resolve_credential(&db.pool, "blake3:fictional", Utc::now())
            .await
            .unwrap()
            .is_none()
    );
    policy::register_credential(
        &db.pool,
        CredentialId::generate(),
        &b.principal,
        "blake3:fictional",
        None,
    )
    .await
    .unwrap();
    assert!(
        policy::resolve_credential(&db.pool, "blake3:fictional", Utc::now())
            .await
            .unwrap()
            .is_none()
    )
}
#[tokio::test]
async fn default_deny_separates_read_curate_tenants_and_remote_egress() {
    let (db, p, _, _) = legacy_db().await;
    let b = policy::bootstrap_personal_policy(&db.pool, p.tenant_id)
        .await
        .unwrap();
    let spec = GrantSpec {
        tenant_id: p.tenant_id,
        workspace_id: b.workspace_id,
        project_id: Some(p.id),
        actions: vec![PolicyAction::ReadKnowledge],
        delegable_actions: vec![],
        classification_ceiling: SecurityClassification::Internal,
        allowed_destinations: vec![],
        budget: ExecutionBudget::default(),
        valid_until: Some(Utc::now() + Duration::hours(1)),
    };
    let service = policy::bootstrap_service_policy(&db.pool, p.tenant_id, "reader", &spec)
        .await
        .unwrap();
    let same = policy::bootstrap_service_policy(&db.pool, p.tenant_id, "reader", &spec)
        .await
        .unwrap();
    assert_eq!(same.principal.id(), service.principal.id());
    assert_eq!(same.grant_id, service.grant_id);
    let resource = policy::resource_for_project(&db.pool, p.tenant_id, p.id)
        .await
        .unwrap();
    assert!(
        decide(
            &db,
            &service.principal,
            &request(resource.clone(), PolicyAction::ReadKnowledge)
        )
        .await
        .is_some()
    );
    assert!(
        decide(
            &db,
            &service.principal,
            &request(resource.clone(), PolicyAction::PromoteKnowledge)
        )
        .await
        .is_none()
    );
    let mut foreign = resource.clone();
    foreign.tenant_id = TenantId::generate();
    assert!(
        decide(
            &db,
            &service.principal,
            &request(foreign, PolicyAction::ReadKnowledge)
        )
        .await
        .is_none()
    );
    let mut egress = request(resource, PolicyAction::ModelEgress);
    egress.destination = Some(ModelDestination::Remote("paid-provider".into()));
    assert!(decide(&db, &b.principal, &egress).await.is_none());
    let audit:(Option<String>,Option<i64>,Option<i32>,bool)=sqlx::query_as("SELECT destination,requested_tokens,requested_items,allowed FROM policy_decisions WHERE action='MODEL_EGRESS' ORDER BY recorded_at DESC LIMIT 1").fetch_one(&db.pool).await.unwrap();
    assert_eq!(
        audit,
        (Some("REMOTE:paid-provider".into()), None, None, false)
    );
}
#[tokio::test]
async fn grant_revocation_expiry_and_classification_are_enforced() {
    let (db, p, _, _) = legacy_db().await;
    let b = policy::bootstrap_personal_policy(&db.pool, p.tenant_id)
        .await
        .unwrap();
    let spec = GrantSpec {
        tenant_id: p.tenant_id,
        workspace_id: b.workspace_id,
        project_id: Some(p.id),
        actions: vec![PolicyAction::ReadKnowledge],
        delegable_actions: vec![],
        classification_ceiling: SecurityClassification::Public,
        allowed_destinations: vec![],
        budget: ExecutionBudget::default(),
        valid_until: Some(Utc::now() - Duration::seconds(1)),
    };
    let identity = policy::bootstrap_service_policy(
        &db.pool,
        p.tenant_id,
        "classification-test",
        &GrantSpec {
            tenant_id: p.tenant_id,
            workspace_id: b.workspace_id,
            project_id: None,
            actions: vec![PolicyAction::CreateProject],
            delegable_actions: vec![],
            classification_ceiling: SecurityClassification::Public,
            allowed_destinations: vec![],
            budget: ExecutionBudget::default(),
            valid_until: Some(Utc::now() + Duration::hours(1)),
        },
    )
    .await
    .unwrap();
    let principal = identity.principal;
    let grant = GrantId::generate();
    policy::insert_grant(
        &db.pool,
        grant,
        &principal,
        &spec,
        Utc::now() - Duration::hours(1),
    )
    .await
    .unwrap();
    let resource = policy::resource_for_project(&db.pool, p.tenant_id, p.id)
        .await
        .unwrap();
    assert!(
        decide(
            &db,
            &principal,
            &request(resource.clone(), PolicyAction::ReadKnowledge)
        )
        .await
        .is_none()
    );
    let mut current = spec;
    current.valid_until = Some(Utc::now() + Duration::hours(1));
    let live = GrantId::generate();
    policy::insert_grant(
        &db.pool,
        live,
        &principal,
        &current,
        Utc::now() - Duration::minutes(1),
    )
    .await
    .unwrap();
    let mut secret = request(resource.clone(), PolicyAction::ReadKnowledge);
    secret.classification = SecurityClassification::Secret;
    assert!(decide(&db, &principal, &secret).await.is_none());
    assert!(
        decide(
            &db,
            &principal,
            &request(resource.clone(), PolicyAction::ReadKnowledge)
        )
        .await
        .is_none()
    );
    let mut public = request(resource, PolicyAction::ReadKnowledge);
    public.classification = SecurityClassification::Public;
    assert!(decide(&db, &principal, &public).await.is_some());
    policy::revoke_grant(&db.pool, live, Utc::now())
        .await
        .unwrap();
    assert!(decide(&db, &principal, &public).await.is_none())
}
#[tokio::test]
async fn project_binding_is_idempotent_and_domain_safe() {
    let db = fresh_db().await.unwrap();
    let t = TenantId::generate();
    let b = policy::bootstrap_personal_policy(&db.pool, t)
        .await
        .unwrap();
    let p = project(t, "new", OwnershipDomain::Personal);
    projects::insert(&db.pool, &p).await.unwrap();
    policy::bind_project(
        &db.pool,
        t,
        p.id,
        b.workspace_id,
        None,
        OwnershipDomain::Personal,
    )
    .await
    .unwrap();
    policy::bind_project(
        &db.pool,
        t,
        p.id,
        b.workspace_id,
        None,
        OwnershipDomain::Personal,
    )
    .await
    .unwrap();
    assert!(
        policy::bind_project(
            &db.pool,
            t,
            p.id,
            WorkspaceId::generate(),
            None,
            OwnershipDomain::Personal
        )
        .await
        .is_err()
    )
}
#[tokio::test]
async fn child_has_no_inherited_authority_and_must_attenuate() {
    let (db, p, _, _) = legacy_db().await;
    let b = policy::bootstrap_personal_policy(&db.pool, p.tenant_id)
        .await
        .unwrap();
    let until = Utc::now() + Duration::minutes(30);
    let child = GrantSpec {
        tenant_id: p.tenant_id,
        workspace_id: b.workspace_id,
        project_id: Some(p.id),
        actions: vec![PolicyAction::ReadKnowledge],
        delegable_actions: vec![],
        classification_ceiling: SecurityClassification::Internal,
        allowed_destinations: vec![],
        budget: ExecutionBudget {
            max_tokens: Some(100),
            max_items: Some(2),
        },
        valid_until: Some(until),
    };
    let mut tx = db.pool.begin().await.unwrap();
    let execution = policy::create_child_execution(
        &mut tx,
        &b.principal,
        b.grant_id,
        policy::NewChildExecution {
            grant: &child,
            task: "research",
            harness: "local",
            destination: None,
            expires_at: until + Duration::minutes(1),
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let resource = policy::resource_for_project(&db.pool, p.tenant_id, p.id)
        .await
        .unwrap();
    assert!(
        decide(
            &db,
            &execution.principal,
            &request(resource.clone(), PolicyAction::ReadKnowledge)
        )
        .await
        .is_none()
    );
    let mut scoped = request(resource.clone(), PolicyAction::ReadKnowledge);
    scoped.execution_id = Some(execution.execution_id);
    scoped.budget = ExecutionBudget {
        max_tokens: Some(50),
        max_items: Some(1),
    };
    assert!(decide(&db, &execution.principal, &scoped).await.is_some());
    let mut promote = scoped;
    promote.action = PolicyAction::PromoteKnowledge;
    assert!(decide(&db, &execution.principal, &promote).await.is_none());
    let mut human = child.clone();
    human.actions = vec![PolicyAction::AttributeHumanProposal];
    let mut tx = db.pool.begin().await.unwrap();
    assert!(
        policy::create_child_execution(
            &mut tx,
            &b.principal,
            b.grant_id,
            policy::NewChildExecution {
                grant: &human,
                task: "impersonate",
                harness: "local",
                destination: None,
                expires_at: until,
            }
        )
        .await
        .is_err()
    );
    tx.rollback().await.unwrap();
    let mut escalation = child;
    escalation.actions.push(PolicyAction::PromoteKnowledge);
    let mut tx = db.pool.begin().await.unwrap();
    assert!(
        policy::create_child_execution(
            &mut tx,
            &b.principal,
            b.grant_id,
            policy::NewChildExecution {
                grant: &escalation,
                task: "bad",
                harness: "local",
                destination: None,
                expires_at: until + Duration::minutes(1),
            }
        )
        .await
        .is_err()
    );
    tx.rollback().await.unwrap()
}

#[tokio::test]
async fn transaction_local_policy_context_does_not_leak_on_pool_reuse() {
    let db = fresh_db().await.unwrap();
    let tenant = TenantId::generate();
    let owner = policy::bootstrap_personal_policy(&db.pool, tenant)
        .await
        .unwrap();
    let mut connection = db.pool.acquire().await.unwrap();
    let mut tx = connection.begin().await.unwrap();
    policy::set_local_policy_context(&mut tx, &owner.principal)
        .await
        .unwrap();
    let inside: (String, String) = sqlx::query_as(
        "SELECT current_setting('ownstate.tenant_id'),current_setting('ownstate.principal_id')",
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(inside.0, tenant.to_string());
    assert_eq!(inside.1, owner.principal.id().to_string());
    tx.commit().await.unwrap();

    let after: (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT NULLIF(current_setting('ownstate.tenant_id',true),''),NULLIF(current_setting('ownstate.principal_id',true),'')",
    )
    .fetch_one(&mut *connection)
    .await
    .unwrap();
    assert_eq!(after, (None, None));
}
#[tokio::test]
async fn decisions_and_grants_are_append_only() {
    let (db, p, _, _) = legacy_db().await;
    let b = policy::bootstrap_personal_policy(&db.pool, p.tenant_id)
        .await
        .unwrap();
    let r = policy::resource_for_project(&db.pool, p.tenant_id, p.id)
        .await
        .unwrap();
    assert!(
        decide(&db, &b.principal, &request(r, PolicyAction::ReadProject))
            .await
            .is_some()
    );
    let (d,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM policy_decisions LIMIT 1")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert!(
        sqlx::query("DELETE FROM policy_decisions WHERE id=$1")
            .bind(d)
            .execute(&db.pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE authorization_grants SET actions=ARRAY['READ_PROJECT'] WHERE id=$1")
            .bind(b.grant_id.as_uuid())
            .execute(&db.pool)
            .await
            .is_err()
    )
}
