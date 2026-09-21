//! Principal-scoped authorization at the application boundary.
//!
//! Interface adapters authenticate into an opaque `AuthenticatedPrincipal`
//! and use `PrincipalServices`; request bodies never establish identity.

use std::sync::Arc;

use chrono::Utc;
use ownstate_domain::{
    AgentExecutionId, AuthorizedScope, CandidateId, CandidateKnowledge, CredentialId, EntityId,
    EntityResolution, ExecutionBudget, GrantSpec, GraphCandidate, GraphCandidateId,
    InstitutionalVersion, InteractionEvent, KnowledgeItem, KnowledgeItemId, KnowledgeVersion,
    ModelDestination, NewInteractionEvent, OwnershipDomain, PolicyAction, PolicyRequest,
    PrincipalKind, Project, ProjectId, ProposerKind, QueryPlan, ResourceScope,
    SecurityClassification, Session, SessionId, limits,
};
use ownstate_storage::{policy, projects};
use sqlx::{Postgres, Transaction};

pub use ownstate_storage::policy::AuthenticatedPrincipal;

use crate::bootstrap::ProjectBootstrap;
use crate::context::{CompileRequest, CompiledContext};
use crate::institutional::{InstitutionalQuery, InstitutionalSnapshot, ProposeInstitutional};
use crate::knowledge::{KnowledgeDetail, ProposeKnowledge};
use crate::projects::{CreateProject, EnsureProject};
use crate::query::QueryResult;
use crate::search::{SearchHit, SearchParams};
use crate::sessions::CreateSession;
use crate::{AppServices, ServiceError, ServiceResult};

#[derive(Clone)]
pub struct PrincipalServices {
    app: Arc<AppServices>,
    principal: AuthenticatedPrincipal,
    execution_id: Option<AgentExecutionId>,
}

pub struct ChildExecutionRequest {
    pub grant: GrantSpec,
    pub task: String,
    pub harness: String,
    pub destination: Option<ModelDestination>,
    pub expires_at: chrono::DateTime<Utc>,
}

struct PolicyLease {
    tx: Transaction<'static, Postgres>,
    scope: AuthorizedScope,
}

impl PolicyLease {
    async fn finish(self) -> ServiceResult<()> {
        self.tx.commit().await?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct PersonalPolicyActors {
    pub owner: AuthenticatedPrincipal,
    pub mcp: AuthenticatedPrincipal,
    pub worker: AuthenticatedPrincipal,
}

impl AppServices {
    pub fn for_principal(
        self: &Arc<Self>,
        principal: AuthenticatedPrincipal,
    ) -> ServiceResult<PrincipalServices> {
        if principal.tenant_id() != self.tenant_id() {
            return Err(ServiceError::forbidden(
                "principal is outside this deployment tenant",
            ));
        }
        if principal.kind() == PrincipalKind::Agent {
            return Err(ServiceError::forbidden(
                "agent principals require a bound execution",
            ));
        }
        Ok(PrincipalServices {
            app: Arc::clone(self),
            principal,
            execution_id: None,
        })
    }

    fn for_agent_execution(
        self: &Arc<Self>,
        child: policy::ChildExecution,
    ) -> ServiceResult<PrincipalServices> {
        if child.principal.tenant_id() != self.tenant_id()
            || child.principal.kind() != PrincipalKind::Agent
        {
            return Err(ServiceError::forbidden(
                "agent execution is outside this deployment tenant",
            ));
        }
        Ok(PrincipalServices {
            app: Arc::clone(self),
            principal: child.principal,
            execution_id: Some(child.execution_id),
        })
    }

    pub async fn authenticate_digest(
        &self,
        credential_digest: &str,
    ) -> ServiceResult<AuthenticatedPrincipal> {
        policy::resolve_credential(self.pool(), credential_digest, Utc::now())
            .await?
            .ok_or_else(|| ServiceError::forbidden("credential is invalid, expired, or revoked"))
    }

    pub async fn bootstrap_personal_policy_actors(
        &self,
        api_digest: Option<&str>,
        admin_digest: Option<&str>,
    ) -> ServiceResult<PersonalPolicyActors> {
        let owner = policy::bootstrap_personal_policy(self.pool(), self.tenant_id()).await?;
        let service_spec = |actions: Vec<PolicyAction>| GrantSpec {
            tenant_id: self.tenant_id(),
            workspace_id: owner.workspace_id,
            project_id: None,
            actions,
            delegable_actions: Vec::new(),
            classification_ceiling: SecurityClassification::Secret,
            allowed_destinations: vec![
                "LOCAL:fastembed".to_string(),
                "LOCAL:deterministic".to_string(),
            ],
            budget: ExecutionBudget::default(),
            valid_until: None,
        };
        let read_propose = vec![
            PolicyAction::CreateProject,
            PolicyAction::ReadProject,
            PolicyAction::CreateSession,
            PolicyAction::ReadEvidence,
            PolicyAction::AppendEvidence,
            PolicyAction::ReadKnowledge,
            PolicyAction::CompileContext,
            PolicyAction::ProposeKnowledge,
            PolicyAction::GenerateEmbedding,
            PolicyAction::ModelEgress,
        ];
        let mut curator_actions = read_propose.clone();
        curator_actions.extend([
            PolicyAction::AttributeHumanProposal,
            PolicyAction::PromoteKnowledge,
            PolicyAction::RejectCandidate,
        ]);
        let api_actions = if admin_digest.is_none() {
            curator_actions.clone()
        } else {
            read_propose.clone()
        };
        let api = policy::bootstrap_service_policy(
            self.pool(),
            self.tenant_id(),
            if admin_digest.is_none() {
                "api-curator"
            } else {
                "api"
            },
            &service_spec(api_actions),
        )
        .await?;
        let curator = policy::bootstrap_service_policy(
            self.pool(),
            self.tenant_id(),
            "curator",
            &service_spec(curator_actions),
        )
        .await?;
        let mcp = policy::bootstrap_service_policy(
            self.pool(),
            self.tenant_id(),
            "mcp",
            &service_spec(vec![
                PolicyAction::CreateProject,
                PolicyAction::ReadProject,
                PolicyAction::ReadKnowledge,
                PolicyAction::CompileContext,
                PolicyAction::ProposeKnowledge,
                PolicyAction::GenerateEmbedding,
                PolicyAction::ModelEgress,
            ]),
        )
        .await?;
        let worker = policy::bootstrap_service_policy(
            self.pool(),
            self.tenant_id(),
            "worker",
            &service_spec(vec![
                PolicyAction::ReadKnowledge,
                PolicyAction::GenerateEmbedding,
                PolicyAction::ModelEgress,
            ]),
        )
        .await?;

        match (api_digest, admin_digest) {
            (Some(api_hash), Some(admin_hash)) if api_hash == admin_hash => {
                policy::register_credential(
                    self.pool(),
                    CredentialId::generate(),
                    &curator.principal,
                    api_hash,
                    None,
                )
                .await?;
            }
            (api_hash, admin_hash) => {
                if let Some(hash) = api_hash {
                    policy::register_credential(
                        self.pool(),
                        CredentialId::generate(),
                        &api.principal,
                        hash,
                        None,
                    )
                    .await?;
                }
                if let Some(hash) = admin_hash {
                    policy::register_credential(
                        self.pool(),
                        CredentialId::generate(),
                        &curator.principal,
                        hash,
                        None,
                    )
                    .await?;
                }
            }
        }
        Ok(PersonalPolicyActors {
            owner: owner.principal,
            mcp: mcp.principal,
            worker: worker.principal,
        })
    }
}

impl PrincipalServices {
    pub fn principal(&self) -> &AuthenticatedPrincipal {
        &self.principal
    }

    pub(crate) fn app(&self) -> &Arc<AppServices> {
        &self.app
    }

    pub fn execution_id(&self) -> Option<AgentExecutionId> {
        self.execution_id
    }

    async fn begin_authorize(
        &self,
        action: PolicyAction,
        resource: ResourceScope,
        classification: SecurityClassification,
        destination: Option<ModelDestination>,
        budget: ExecutionBudget,
    ) -> ServiceResult<PolicyLease> {
        if resource.tenant_id != self.app.tenant_id()
            || self.principal.tenant_id() != resource.tenant_id
        {
            return Err(ServiceError::forbidden(
                "resource is outside principal scope",
            ));
        }
        let request = PolicyRequest {
            action,
            resource,
            classification,
            destination,
            budget,
            now: Utc::now(),
            execution_id: self.execution_id,
        };
        let mut tx = self.app.pool().begin().await?;
        policy::set_local_policy_context(&mut tx, &self.principal).await?;
        let decision = policy::authorize_and_record(&mut tx, &self.principal, &request).await?;
        match decision {
            Some(scope) => Ok(PolicyLease { tx, scope }),
            None => {
                tx.commit().await?;
                Err(ServiceError::forbidden(
                    "authorization policy denied this action",
                ))
            }
        }
    }

    pub async fn authorize(
        &self,
        action: PolicyAction,
        resource: ResourceScope,
        classification: SecurityClassification,
        destination: Option<ModelDestination>,
        budget: ExecutionBudget,
    ) -> ServiceResult<AuthorizedScope> {
        let lease = self
            .begin_authorize(action, resource, classification, destination, budget)
            .await?;
        let scope = lease.scope.clone();
        lease.finish().await?;
        Ok(scope)
    }

    pub async fn create_child_execution(
        &self,
        req: ChildExecutionRequest,
    ) -> ServiceResult<PrincipalServices> {
        let destination_key = req.destination.as_ref().map(ModelDestination::policy_key);
        let lease = self
            .begin_authorize(
                PolicyAction::DelegateAgent,
                ResourceScope {
                    tenant_id: req.grant.tenant_id,
                    workspace_id: req.grant.workspace_id,
                    project_id: req.grant.project_id,
                },
                req.grant.classification_ceiling,
                req.destination.clone(),
                req.grant.budget,
            )
            .await?;
        let mut tx = self.app.pool().begin().await?;
        policy::set_local_policy_context(&mut tx, &self.principal).await?;
        let child = policy::create_child_execution(
            &mut tx,
            &self.principal,
            lease.scope.grant_id(),
            policy::NewChildExecution {
                grant: &req.grant,
                task: &req.task,
                harness: &req.harness,
                destination: destination_key.as_deref(),
                expires_at: req.expires_at,
            },
        )
        .await?;
        tx.commit().await?;
        lease.finish().await?;
        self.app.for_agent_execution(child)
    }

    pub(crate) async fn authorize_resource(
        &self,
        action: PolicyAction,
        resource: ResourceScope,
        classification: SecurityClassification,
    ) -> ServiceResult<AuthorizedScope> {
        self.authorize(
            action,
            resource,
            classification,
            None,
            ExecutionBudget::default(),
        )
        .await
    }

    pub(crate) async fn authorize_local_embedding(
        &self,
        resource: ResourceScope,
        classification: SecurityClassification,
        budget: ExecutionBudget,
    ) -> ServiceResult<()> {
        let (generate, egress) = self
            .begin_local_embedding(resource, classification, budget)
            .await?;
        generate.finish().await?;
        egress.finish().await?;
        Ok(())
    }

    async fn begin_local_embedding(
        &self,
        resource: ResourceScope,
        classification: SecurityClassification,
        budget: ExecutionBudget,
    ) -> ServiceResult<(PolicyLease, PolicyLease)> {
        let provider = match self.app.embedder().model_name() {
            "deterministic-hash" => "deterministic",
            _ => "fastembed",
        };
        let destination = ModelDestination::Local(provider.to_string());
        let generate = self
            .begin_authorize(
                PolicyAction::GenerateEmbedding,
                resource.clone(),
                classification,
                None,
                budget,
            )
            .await?;
        let egress = self
            .begin_authorize(
                PolicyAction::ModelEgress,
                resource,
                classification,
                Some(destination),
                budget,
            )
            .await?;
        Ok((generate, egress))
    }

    pub async fn get_project(&self, id: ProjectId) -> ServiceResult<Project> {
        let resource =
            policy::resource_for_project(self.app.pool(), self.app.tenant_id(), id).await?;
        let lease = self
            .begin_authorize(
                PolicyAction::ReadProject,
                resource,
                SecurityClassification::Public,
                None,
                ExecutionBudget::default(),
            )
            .await?;
        let result = self.app.get_project(id).await;
        lease.finish().await?;
        result
    }

    pub async fn create_project(&self, req: CreateProject) -> ServiceResult<Project> {
        let bootstrap =
            policy::bootstrap_personal_policy(self.app.pool(), self.app.tenant_id()).await?;
        if req.ownership_domain == Some(OwnershipDomain::Organization) {
            return Err(ServiceError::forbidden(
                "organization projects require an explicitly authorized organization workspace",
            ));
        }
        let name = req.name.trim().to_string();
        if name.is_empty() || name.len() > limits::MAX_NAME_LEN {
            return Err(ServiceError::validation(format!(
                "project name must be 1..={} characters",
                limits::MAX_NAME_LEN
            )));
        }
        let request = PolicyRequest {
            action: PolicyAction::CreateProject,
            resource: ResourceScope {
                tenant_id: self.app.tenant_id(),
                workspace_id: bootstrap.workspace_id,
                project_id: None,
            },
            classification: SecurityClassification::Internal,
            destination: None,
            budget: ExecutionBudget::default(),
            now: Utc::now(),
            execution_id: self.execution_id,
        };
        let project = Project {
            id: ProjectId::generate(),
            tenant_id: self.app.tenant_id(),
            ownership_domain: OwnershipDomain::Personal,
            name,
            description: req.description,
            metadata: req
                .metadata
                .unwrap_or_else(|| serde_json::Value::Object(Default::default())),
            created_at: Utc::now(),
        };
        let mut tx = self.app.pool().begin().await?;
        policy::set_local_policy_context(&mut tx, &self.principal).await?;
        let allowed = policy::authorize_and_record(&mut tx, &self.principal, &request).await?;
        if allowed.is_none() {
            tx.commit().await?;
            return Err(ServiceError::forbidden(
                "authorization policy denied project creation",
            ));
        }
        match projects::insert(&mut *tx, &project).await {
            Ok(()) => {}
            Err(error) if error.is_unique_violation() => {
                return Err(ServiceError::Conflict(format!(
                    "a project named '{}' already exists",
                    project.name
                )));
            }
            Err(error) => return Err(error.into()),
        }
        policy::bind_project(
            &mut *tx,
            project.tenant_id,
            project.id,
            bootstrap.workspace_id,
            None,
            OwnershipDomain::Personal,
        )
        .await?;
        tx.commit().await?;
        Ok(project)
    }

    pub async fn ensure_project(&self, req: EnsureProject) -> ServiceResult<Project> {
        let bootstrap =
            policy::bootstrap_personal_policy(self.app.pool(), self.app.tenant_id()).await?;
        let mut existing = None;
        if let Some(origin) = &req.git_origin {
            existing =
                projects::find_by_git_origin(self.app.pool(), self.app.tenant_id(), origin).await?;
        }
        if existing.is_none()
            && let Some(path) = &req.root_path
        {
            existing =
                projects::find_by_root_path(self.app.pool(), self.app.tenant_id(), path).await?;
        }
        if existing.is_none() {
            existing =
                projects::find_by_name(self.app.pool(), self.app.tenant_id(), req.name.trim())
                    .await?;
        }
        let lease = if let Some(project) = existing {
            let scope =
                policy::resource_for_project(self.app.pool(), self.app.tenant_id(), project.id)
                    .await?;
            self.begin_authorize(
                PolicyAction::ReadProject,
                scope,
                SecurityClassification::Internal,
                None,
                ExecutionBudget::default(),
            )
            .await?
        } else {
            self.begin_authorize(
                PolicyAction::CreateProject,
                ResourceScope {
                    tenant_id: self.app.tenant_id(),
                    workspace_id: bootstrap.workspace_id,
                    project_id: None,
                },
                SecurityClassification::Internal,
                None,
                ExecutionBudget::default(),
            )
            .await?
        };
        let result = self.app.ensure_project(req).await;
        lease.finish().await?;
        result
    }

    pub async fn create_session(&self, req: CreateSession) -> ServiceResult<Session> {
        let resource =
            policy::resource_for_project(self.app.pool(), self.app.tenant_id(), req.project_id)
                .await?;
        let lease = self
            .begin_authorize(
                PolicyAction::CreateSession,
                resource,
                SecurityClassification::Internal,
                None,
                ExecutionBudget::default(),
            )
            .await?;
        let result = self.app.create_session(req).await;
        lease.finish().await?;
        result
    }

    pub async fn get_session(&self, id: SessionId) -> ServiceResult<Session> {
        let resource =
            policy::resource_for_session(self.app.pool(), self.app.tenant_id(), id).await?;
        let lease = self
            .begin_authorize(
                PolicyAction::ReadEvidence,
                resource,
                SecurityClassification::Internal,
                None,
                ExecutionBudget::default(),
            )
            .await?;
        let result = self.app.get_session(id).await;
        lease.finish().await?;
        result
    }

    pub async fn append_events(
        &self,
        id: SessionId,
        events: Vec<NewInteractionEvent>,
    ) -> ServiceResult<Vec<InteractionEvent>> {
        let resource =
            policy::resource_for_session(self.app.pool(), self.app.tenant_id(), id).await?;
        let classification = events
            .iter()
            .filter_map(|event| event.security_classification)
            .max()
            .unwrap_or(SecurityClassification::Internal);
        let lease = self
            .begin_authorize(
                PolicyAction::AppendEvidence,
                resource,
                classification,
                None,
                ExecutionBudget::default(),
            )
            .await?;
        let result = self.app.append_events(id, events).await;
        lease.finish().await?;
        result
    }

    pub async fn list_events(&self, id: SessionId) -> ServiceResult<Vec<InteractionEvent>> {
        let resource =
            policy::resource_for_session(self.app.pool(), self.app.tenant_id(), id).await?;
        let lease = self
            .begin_authorize(
                PolicyAction::ReadEvidence,
                resource,
                self.app.max_classification(),
                None,
                ExecutionBudget::default(),
            )
            .await?;
        let result = self.app.list_events(id).await;
        lease.finish().await?;
        result
    }

    pub async fn propose_knowledge(
        &self,
        req: ProposeKnowledge,
    ) -> ServiceResult<CandidateKnowledge> {
        let resource =
            policy::resource_for_project(self.app.pool(), self.app.tenant_id(), req.project_id)
                .await?;
        let classification = req
            .security_classification
            .unwrap_or(SecurityClassification::Internal);
        let proposal = self
            .begin_authorize(
                PolicyAction::ProposeKnowledge,
                resource.clone(),
                classification,
                None,
                ExecutionBudget::default(),
            )
            .await?;
        let attribution = if req.proposed_by == ProposerKind::Human {
            Some(
                self.begin_authorize(
                    PolicyAction::AttributeHumanProposal,
                    resource,
                    classification,
                    None,
                    ExecutionBudget::default(),
                )
                .await?,
            )
        } else {
            None
        };
        let result = self.app.propose_knowledge(req).await;
        proposal.finish().await?;
        if let Some(lease) = attribution {
            lease.finish().await?;
        }
        result
    }

    pub async fn get_candidate(&self, id: CandidateId) -> ServiceResult<CandidateKnowledge> {
        let resource =
            policy::resource_for_candidate(self.app.pool(), self.app.tenant_id(), id).await?;
        let lease = self
            .begin_authorize(
                PolicyAction::ReadKnowledge,
                resource,
                self.app.max_classification(),
                None,
                ExecutionBudget::default(),
            )
            .await?;
        let result = self.app.get_candidate(id).await;
        lease.finish().await?;
        result
    }

    pub async fn promote_candidate(
        &self,
        id: CandidateId,
    ) -> ServiceResult<(KnowledgeItem, KnowledgeVersion)> {
        let resource =
            policy::resource_for_candidate(self.app.pool(), self.app.tenant_id(), id).await?;
        let lease = self
            .begin_authorize(
                PolicyAction::PromoteKnowledge,
                resource,
                self.app.max_classification(),
                None,
                ExecutionBudget::default(),
            )
            .await?;
        let result = self.app.promote_candidate(id).await;
        lease.finish().await?;
        result
    }

    pub async fn reject_candidate(&self, id: CandidateId, reason: &str) -> ServiceResult<()> {
        let resource =
            policy::resource_for_candidate(self.app.pool(), self.app.tenant_id(), id).await?;
        let lease = self
            .begin_authorize(
                PolicyAction::RejectCandidate,
                resource,
                self.app.max_classification(),
                None,
                ExecutionBudget::default(),
            )
            .await?;
        let result = self.app.reject_candidate(id, reason).await;
        lease.finish().await?;
        result
    }

    pub async fn search_knowledge(&self, mut req: SearchParams) -> ServiceResult<Vec<SearchHit>> {
        let resource =
            policy::resource_for_project(self.app.pool(), self.app.tenant_id(), req.project_id)
                .await?;
        let allowed = self
            .authorize_resource(
                PolicyAction::ReadKnowledge,
                resource.clone(),
                req.max_classification,
            )
            .await?;
        req.max_classification = req
            .max_classification
            .min(allowed.classification_ceiling)
            .min(self.app.max_classification());
        self.authorize_local_embedding(
            resource.clone(),
            req.max_classification,
            ExecutionBudget {
                max_tokens: None,
                max_items: Some(req.limit as i32),
            },
        )
        .await?;
        let result = self.app.search_knowledge(req).await?;
        self.authorize_resource(
            PolicyAction::ReadKnowledge,
            resource,
            allowed.classification_ceiling,
        )
        .await?;
        Ok(result)
    }

    pub async fn get_knowledge(&self, id: KnowledgeItemId) -> ServiceResult<KnowledgeDetail> {
        let resource =
            policy::resource_for_knowledge_item(self.app.pool(), self.app.tenant_id(), id).await?;
        let lease = self
            .begin_authorize(
                PolicyAction::ReadKnowledge,
                resource,
                self.app.max_classification(),
                None,
                ExecutionBudget::default(),
            )
            .await?;
        let result = self.app.get_knowledge(id).await;
        lease.finish().await?;
        result
    }

    pub async fn compile_context(&self, mut req: CompileRequest) -> ServiceResult<CompiledContext> {
        let resource =
            policy::resource_for_project(self.app.pool(), self.app.tenant_id(), req.project_id)
                .await?;
        let requested = req
            .max_classification
            .unwrap_or(self.app.max_classification());
        let compile = self
            .begin_authorize(
                PolicyAction::CompileContext,
                resource.clone(),
                requested,
                None,
                ExecutionBudget {
                    max_tokens: req.token_budget,
                    max_items: req.max_items.map(|v| v as i32),
                },
            )
            .await?;
        req.max_classification = Some(requested.min(compile.scope.classification_ceiling));
        let embedding_budget = ExecutionBudget {
            max_tokens: req.token_budget,
            max_items: req.max_items.map(|v| v as i32),
        };
        let (generate, egress) = self
            .begin_local_embedding(
                resource,
                req.max_classification.unwrap_or(requested),
                embedding_budget,
            )
            .await?;
        let result = self.app.compile_context(req).await;
        compile.finish().await?;
        generate.finish().await?;
        egress.finish().await?;
        result
    }

    pub async fn execute_query(&self, plan: QueryPlan) -> ServiceResult<QueryResult> {
        let constraints = plan.constraints().clone();
        let resource = policy::resource_for_project(
            self.app.pool(),
            self.app.tenant_id(),
            constraints.project_id,
        )
        .await?;
        self.authorize_resource(
            PolicyAction::ReadKnowledge,
            resource.clone(),
            constraints.max_classification,
        )
        .await?;
        if matches!(
            &plan,
            QueryPlan::Semantic { .. }
                | QueryPlan::Hybrid { .. }
                | QueryPlan::Temporal {
                    subject: ownstate_domain::TemporalSubject::Semantic { .. },
                    ..
                }
        ) {
            self.authorize_local_embedding(
                resource.clone(),
                constraints.max_classification,
                ExecutionBudget {
                    max_tokens: None,
                    max_items: Some(constraints.limit as i32),
                },
            )
            .await?;
        }
        let result = self.app.execute_query(plan).await?;
        self.authorize_resource(
            PolicyAction::ReadKnowledge,
            resource,
            constraints.max_classification,
        )
        .await?;
        Ok(result)
    }

    pub async fn bootstrap_project(
        &self,
        project_id: ProjectId,
        max_items: usize,
        agent: Option<String>,
    ) -> ServiceResult<ProjectBootstrap> {
        let resource =
            policy::resource_for_project(self.app.pool(), self.app.tenant_id(), project_id).await?;
        let read = self
            .begin_authorize(
                PolicyAction::ReadKnowledge,
                resource.clone(),
                self.app.max_classification(),
                None,
                ExecutionBudget {
                    max_tokens: None,
                    max_items: Some(max_items as i32),
                },
            )
            .await?;
        let (generate, egress) = self
            .begin_local_embedding(
                resource,
                self.app.max_classification(),
                ExecutionBudget {
                    max_tokens: None,
                    max_items: Some(max_items as i32),
                },
            )
            .await?;
        let result = self
            .app
            .bootstrap_project(project_id, max_items, agent)
            .await;
        read.finish().await?;
        generate.finish().await?;
        egress.finish().await?;
        result
    }

    pub async fn propose_institutional(
        &self,
        req: ProposeInstitutional,
    ) -> ServiceResult<GraphCandidate> {
        let resource =
            policy::resource_for_project(self.app.pool(), self.app.tenant_id(), req.project_id)
                .await?;
        let classification = req
            .security_classification
            .unwrap_or(SecurityClassification::Internal);
        let proposal = self
            .begin_authorize(
                PolicyAction::ProposeKnowledge,
                resource.clone(),
                classification,
                None,
                ExecutionBudget::default(),
            )
            .await?;
        let attribution = if req.proposed_by == ProposerKind::Human {
            Some(
                self.begin_authorize(
                    PolicyAction::AttributeHumanProposal,
                    resource,
                    classification,
                    None,
                    ExecutionBudget::default(),
                )
                .await?,
            )
        } else {
            None
        };
        let result = self.app.propose_institutional(req).await;
        proposal.finish().await?;
        if let Some(lease) = attribution {
            lease.finish().await?;
        }
        result
    }

    pub async fn promote_institutional(
        &self,
        id: GraphCandidateId,
    ) -> ServiceResult<InstitutionalVersion> {
        let resource =
            policy::resource_for_institutional_candidate(self.app.pool(), self.app.tenant_id(), id)
                .await?;
        let lease = self
            .begin_authorize(
                PolicyAction::PromoteKnowledge,
                resource,
                self.app.max_classification(),
                None,
                ExecutionBudget::default(),
            )
            .await?;
        let result = self.app.promote_institutional(id).await;
        lease.finish().await?;
        result
    }

    pub async fn institutional_snapshot(
        &self,
        query: InstitutionalQuery,
        record_id: Option<uuid::Uuid>,
        record_type: Option<&str>,
    ) -> ServiceResult<InstitutionalSnapshot> {
        let resource =
            policy::resource_for_project(self.app.pool(), self.app.tenant_id(), query.project_id)
                .await?;
        let lease = self
            .begin_authorize(
                PolicyAction::ReadKnowledge,
                resource,
                self.app.max_classification(),
                None,
                ExecutionBudget::default(),
            )
            .await?;
        let result = self
            .app
            .institutional_snapshot(query, record_id, record_type)
            .await;
        lease.finish().await?;
        result
    }

    pub async fn entity_relationships(
        &self,
        project_id: ProjectId,
        entity_id: EntityId,
        limit: usize,
        as_of: Option<chrono::DateTime<Utc>>,
    ) -> ServiceResult<Vec<InstitutionalVersion>> {
        let resource =
            policy::resource_for_project(self.app.pool(), self.app.tenant_id(), project_id).await?;
        let lease = self
            .begin_authorize(
                PolicyAction::ReadKnowledge,
                resource,
                self.app.max_classification(),
                None,
                ExecutionBudget {
                    max_tokens: None,
                    max_items: Some(limit as i32),
                },
            )
            .await?;
        let result = self
            .app
            .entity_relationships(project_id, entity_id, limit, as_of)
            .await;
        lease.finish().await?;
        result
    }

    pub async fn resolve_entity(
        &self,
        project_id: ProjectId,
        value: &str,
        namespace: Option<&str>,
        as_of: Option<chrono::DateTime<Utc>>,
    ) -> ServiceResult<EntityResolution> {
        let resource =
            policy::resource_for_project(self.app.pool(), self.app.tenant_id(), project_id).await?;
        let lease = self
            .begin_authorize(
                PolicyAction::ReadKnowledge,
                resource,
                self.app.max_classification(),
                None,
                ExecutionBudget::default(),
            )
            .await?;
        let result = self
            .app
            .resolve_entity(project_id, value, namespace, as_of)
            .await;
        lease.finish().await?;
        result
    }

    pub async fn institutional_history(
        &self,
        project_id: ProjectId,
        record_id: uuid::Uuid,
    ) -> ServiceResult<Vec<InstitutionalVersion>> {
        let resource =
            policy::resource_for_project(self.app.pool(), self.app.tenant_id(), project_id).await?;
        let lease = self
            .begin_authorize(
                PolicyAction::ReadKnowledge,
                resource,
                self.app.max_classification(),
                None,
                ExecutionBudget::default(),
            )
            .await?;
        let result = self.app.institutional_history(project_id, record_id).await;
        lease.finish().await?;
        result
    }
}
