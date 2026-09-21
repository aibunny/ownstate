#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use ownstate_domain::*;
use ownstate_embeddings::DeterministicProvider;
use ownstate_services::AppServices;
use ownstate_services::knowledge::ProposeKnowledge;
use ownstate_services::policy::PrincipalServices;
use ownstate_services::projects::CreateProject;
use ownstate_services::sessions::CreateSession;
use ownstate_storage::test_support::{TestDb, fresh_db};

pub struct TestServices {
    app: Arc<AppServices>,
    scoped: PrincipalServices,
}

impl TestServices {
    pub async fn from_app(app: Arc<AppServices>) -> Self {
        let owner = app
            .bootstrap_personal_policy_actors(None, None)
            .await
            .unwrap()
            .owner;
        let scoped = app.for_principal(owner).unwrap();
        Self { app, scoped }
    }

    #[allow(dead_code)]
    pub fn pool(&self) -> &sqlx::PgPool {
        self.app.pool()
    }

    #[allow(dead_code)]
    pub fn tenant_id(&self) -> TenantId {
        self.app.tenant_id()
    }
}

impl std::ops::Deref for TestServices {
    type Target = PrincipalServices;

    fn deref(&self) -> &Self::Target {
        &self.scoped
    }
}

pub async fn services() -> (TestServices, TestDb) {
    let db = fresh_db().await.unwrap();
    let app = Arc::new(AppServices::new(
        db.pool.clone(),
        Arc::new(DeterministicProvider::new()),
        TenantId::from_uuid(uuid::Uuid::from_u128(1)),
    ));
    (TestServices::from_app(app).await, db)
}

pub async fn make_project(services: &PrincipalServices, name: &str) -> Project {
    services
        .create_project(CreateProject {
            name: name.into(),
            description: None,
            ownership_domain: None,
            metadata: None,
        })
        .await
        .unwrap()
}

pub async fn make_session(services: &PrincipalServices, project: &Project) -> Session {
    services
        .create_session(CreateSession {
            project_id: project.id,
            source: "test-agent".into(),
            source_session_id: None,
            agent: Some("test".into()),
            provider: None,
            model: None,
            repository: None,
            initial_commit: None,
            metadata: None,
        })
        .await
        .unwrap()
}

pub fn message_event(content: &str) -> NewInteractionEvent {
    NewInteractionEvent {
        event_type: InteractionEventType::AssistantMessage,
        actor_type: ActorType::Assistant,
        actor_id: None,
        sequence: None,
        occurred_at: None,
        source_event_id: None,
        model_provider: None,
        model_name: None,
        content: Some(content.to_string()),
        tool_name: None,
        tool_call_id: None,
        repository: None,
        branch: None,
        commit_sha: None,
        file_path: None,
        security_classification: None,
        metadata: None,
    }
}

pub fn proposal(
    project: &Project,
    subject: &str,
    content: &str,
    evidence: Vec<EventId>,
) -> ProposeKnowledge {
    ProposeKnowledge {
        project_id: project.id,
        session_id: None,
        kind: KnowledgeKind::Architecture,
        subject_key: subject.into(),
        content: content.into(),
        structured_content: None,
        confidence: Some(0.9),
        proposed_by: ProposerKind::Agent,
        source: Some("test".into()),
        evidence_event_ids: evidence,
        security_classification: None,
    }
}
