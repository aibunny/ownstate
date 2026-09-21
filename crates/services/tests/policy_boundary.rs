//! Principal boundary and revocation-race acceptance against real PostgreSQL.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use chrono::{Duration, Utc};
use ownstate_domain::*;
use ownstate_embeddings::{EmbeddingError, EmbeddingProvider};
use ownstate_services::policy::ChildExecutionRequest;
use ownstate_services::policy::PrincipalServices;
use ownstate_services::projects::CreateProject;
use ownstate_services::search::SearchParams;
use ownstate_services::{AppServices, ServiceError};
use ownstate_storage::policy;
use ownstate_storage::test_support::fresh_db;
use tokio::sync::Notify;

struct CountingProvider {
    calls: AtomicUsize,
}

impl CountingProvider {
    fn new() -> Self {
        Self {
            calls: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl EmbeddingProvider for CountingProvider {
    fn model_name(&self) -> &str {
        "deterministic-hash"
    }

    fn model_version(&self) -> &str {
        "policy-test"
    }

    fn dimensions(&self) -> usize {
        384
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(texts.iter().map(|_| vec![0.0; 384]).collect())
    }
}

struct BlockingProvider {
    entered: Notify,
    release: Notify,
}

impl BlockingProvider {
    fn new() -> Self {
        Self {
            entered: Notify::new(),
            release: Notify::new(),
        }
    }
}

#[async_trait]
impl EmbeddingProvider for BlockingProvider {
    fn model_name(&self) -> &str {
        "deterministic-hash"
    }

    fn model_version(&self) -> &str {
        "policy-blocking-test"
    }

    fn dimensions(&self) -> usize {
        384
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        self.entered.notify_one();
        self.release.notified().await;
        Ok(texts.iter().map(|_| vec![0.0; 384]).collect())
    }
}

async fn project(svc: &PrincipalServices, name: &str) -> Project {
    svc.create_project(CreateProject {
        name: name.into(),
        description: None,
        ownership_domain: None,
        metadata: None,
    })
    .await
    .unwrap()
}

fn read_spec(
    tenant_id: TenantId,
    workspace_id: WorkspaceId,
    project_id: ProjectId,
    embedding: bool,
) -> GrantSpec {
    let mut actions = vec![PolicyAction::ReadKnowledge];
    if embedding {
        actions.extend([PolicyAction::GenerateEmbedding, PolicyAction::ModelEgress]);
    }
    GrantSpec {
        tenant_id,
        workspace_id,
        project_id: Some(project_id),
        actions,
        delegable_actions: vec![],
        classification_ceiling: SecurityClassification::Confidential,
        allowed_destinations: embedding
            .then_some(vec!["LOCAL:deterministic".into()])
            .unwrap_or_default(),
        budget: ExecutionBudget::default(),
        valid_until: Some(Utc::now() + Duration::hours(1)),
    }
}

#[tokio::test]
async fn project_scope_and_model_permissions_deny_before_embedding() {
    let db = fresh_db().await.unwrap();
    let tenant = TenantId::generate();
    let provider = Arc::new(CountingProvider::new());
    let app = Arc::new(AppServices::new(db.pool.clone(), provider.clone(), tenant));
    let owner = app
        .bootstrap_personal_policy_actors(None, None)
        .await
        .unwrap()
        .owner;
    let s = app.for_principal(owner).unwrap();
    let allowed_project = project(&s, "allowed").await;
    let denied_project = project(&s, "denied").await;
    let bootstrap = policy::bootstrap_personal_policy(&db.pool, tenant)
        .await
        .unwrap();
    let reader = policy::bootstrap_service_policy(
        &db.pool,
        tenant,
        "policy-reader",
        &read_spec(tenant, bootstrap.workspace_id, allowed_project.id, false),
    )
    .await
    .unwrap();
    let scoped = app.for_principal(reader.principal).unwrap();

    for project_id in [allowed_project.id, denied_project.id] {
        let result = scoped
            .search_knowledge(SearchParams {
                project_id,
                query: "anything".into(),
                kinds: None,
                limit: 3,
                max_classification: SecurityClassification::Internal,
            })
            .await;
        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("policy unexpectedly allowed search"),
        };
        assert!(matches!(error, ServiceError::Forbidden(_)));
    }
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn revocation_during_embedding_suppresses_the_final_read() {
    let db = fresh_db().await.unwrap();
    let tenant = TenantId::generate();
    let provider = Arc::new(BlockingProvider::new());
    let app = Arc::new(AppServices::new(db.pool.clone(), provider.clone(), tenant));
    let owner = app
        .bootstrap_personal_policy_actors(None, None)
        .await
        .unwrap()
        .owner;
    let s = app.for_principal(owner).unwrap();
    let target = project(&s, "revocation").await;
    let bootstrap = policy::bootstrap_personal_policy(&db.pool, tenant)
        .await
        .unwrap();
    let reader = policy::bootstrap_service_policy(
        &db.pool,
        tenant,
        "revocable-reader",
        &read_spec(tenant, bootstrap.workspace_id, target.id, true),
    )
    .await
    .unwrap();
    let scoped = app.for_principal(reader.principal).unwrap();
    let search = tokio::spawn(async move {
        scoped
            .search_knowledge(SearchParams {
                project_id: target.id,
                query: "revoked while embedding".into(),
                kinds: None,
                limit: 3,
                max_classification: SecurityClassification::Internal,
            })
            .await
    });

    provider.entered.notified().await;
    assert!(
        policy::revoke_grant(&db.pool, reader.grant_id, Utc::now())
            .await
            .unwrap()
    );
    provider.release.notify_one();
    let error = match search.await.unwrap() {
        Err(error) => error,
        Ok(_) => panic!("revoked read unexpectedly returned results"),
    };
    assert!(matches!(error, ServiceError::Forbidden(_)));
}

#[tokio::test]
async fn child_services_require_bound_execution_and_explicit_attenuation() {
    let db = fresh_db().await.unwrap();
    let tenant = TenantId::generate();
    let app = Arc::new(AppServices::new(
        db.pool.clone(),
        Arc::new(CountingProvider::new()),
        tenant,
    ));
    let owner = app
        .bootstrap_personal_policy_actors(None, None)
        .await
        .unwrap()
        .owner;
    let s = app.for_principal(owner).unwrap();
    let target = project(&s, "delegation").await;
    let bootstrap = policy::bootstrap_personal_policy(&db.pool, tenant)
        .await
        .unwrap();
    let owner_services = app.for_principal(bootstrap.principal).unwrap();
    let until = Utc::now() + Duration::minutes(10);
    let child = owner_services
        .create_child_execution(ChildExecutionRequest {
            grant: GrantSpec {
                tenant_id: tenant,
                workspace_id: bootstrap.workspace_id,
                project_id: Some(target.id),
                actions: vec![PolicyAction::ReadProject],
                delegable_actions: vec![],
                classification_ceiling: SecurityClassification::Internal,
                allowed_destinations: vec![],
                budget: ExecutionBudget::default(),
                valid_until: Some(until),
            },
            task: "inspect one project".into(),
            harness: "acceptance".into(),
            destination: None,
            expires_at: until,
        })
        .await
        .unwrap();

    assert!(child.execution_id().is_some());
    assert_eq!(child.get_project(target.id).await.unwrap().id, target.id);
    let scope = policy::resource_for_project(&db.pool, tenant, target.id)
        .await
        .unwrap();
    assert!(
        child
            .authorize(
                PolicyAction::PromoteKnowledge,
                scope,
                SecurityClassification::Internal,
                None,
                ExecutionBudget::default(),
            )
            .await
            .is_err()
    );
}
