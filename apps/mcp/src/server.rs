use ownstate_domain::{
    ActorType, CandidateId, EventId, InteractionEventType, KnowledgeItemId, KnowledgeKind,
    ProjectId, ProposerKind, SecurityClassification, SessionId,
};
use ownstate_services::ServiceError;
use ownstate_services::policy::PrincipalServices;
use ownstate_services::projects::EnsureProject;
use ownstate_services::search::SearchParams;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ErrorData, ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::workspace::Workspace;

#[derive(Clone)]
pub struct OwnstateMcp {
    services: PrincipalServices,
    /// Detected at startup from the directory the MCP client launched us in.
    workspace: Option<Workspace>,
}

fn to_mcp_error(err: ServiceError) -> ErrorData {
    match err {
        ServiceError::NotFound(entity) => {
            ErrorData::invalid_params(format!("{entity} not found"), None)
        }
        ServiceError::Validation(msg) => ErrorData::invalid_params(msg, None),
        ServiceError::Conflict(msg) => ErrorData::invalid_params(msg, None),
        ServiceError::Forbidden(msg) => ErrorData::invalid_params(msg, None),
        ServiceError::Embedding(msg) => {
            tracing::error!(error = %msg, "embedding failure in MCP tool");
            ErrorData::internal_error("embedding provider unavailable", None)
        }
        ServiceError::Internal(source) => {
            tracing::error!(error = %source, "internal error in MCP tool");
            ErrorData::internal_error("internal error", None)
        }
    }
}

fn parse_id<T: std::str::FromStr>(raw: &str, what: &str) -> Result<T, ErrorData> {
    raw.parse::<T>()
        .map_err(|_| ErrorData::invalid_params(format!("{what} must be a UUID"), None))
}

fn to_json_text<T: serde::Serialize>(value: &T) -> Result<String, ErrorData> {
    serde_json::to_string_pretty(value)
        .map_err(|e| ErrorData::internal_error(format!("serialization failed: {e}"), None))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BootstrapProjectParams {
    /// Project id (UUID). Usually omitted: the project is resolved from the
    /// current workspace directory name (created on first use).
    pub project_id: Option<String>,
    /// Project name to resolve/create instead of the workspace directory name.
    pub project: Option<String>,
    /// Maximum knowledge entries to include (default 20).
    pub max_items: Option<usize>,
    /// Name of the requesting agent, recorded in the context packet audit.
    pub agent: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchKnowledgeParams {
    /// Project id (UUID). Usually omitted: the project is resolved from the
    /// current workspace directory name (created on first use).
    pub project_id: Option<String>,
    /// Project name to resolve/create instead of the workspace directory name.
    pub project: Option<String>,
    /// Search query describing what you need to know.
    pub query: String,
    /// Maximum results (default 10).
    pub limit: Option<usize>,
    /// Restrict to knowledge kinds (e.g. ["ARCHITECTURE", "DECISION"]).
    pub kinds: Option<Vec<String>>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetKnowledgeParams {
    /// Knowledge item id (UUID).
    pub knowledge_item_id: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetEntityParams {
    pub project_id: String,
    pub entity_id: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetRelationshipsParams {
    pub project_id: String,
    pub entity_id: String,
    pub limit: Option<usize>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompileContextParams {
    pub project_id: String,
    pub task: String,
    pub max_items: Option<usize>,
    pub token_budget: Option<i64>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub agent: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposeKnowledgeParams {
    /// Project id (UUID). Usually omitted: the project is resolved from the
    /// current workspace directory name (created on first use).
    pub project_id: Option<String>,
    /// Project name to resolve/create instead of the workspace directory name.
    pub project: Option<String>,
    /// Session id (UUID) this knowledge was learned in, when applicable.
    pub session_id: Option<String>,
    /// Knowledge kind (FACT, ARCHITECTURE, DECISION, ...).
    pub kind: String,
    /// Stable subject key identifying what the knowledge is about
    /// (e.g. "authentication-architecture").
    pub subject_key: String,
    /// The knowledge claim itself.
    pub content: String,
    /// Extractor confidence in [0, 1].
    pub confidence: Option<f32>,
    /// Interaction event ids (UUIDs) that evidence this claim.
    pub evidence_event_ids: Option<Vec<String>>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecordParams {
    /// Project id (UUID). Usually omitted: resolved from workspace.
    pub project_id: Option<String>,
    /// Project name to resolve/create instead of the workspace directory name.
    pub project: Option<String>,
    /// Event type (USER_MESSAGE, TOOL_CALL, FILE_WRITE, etc.).
    pub event_type: String,
    /// Actor type (USER, ASSISTANT, TOOL, SYSTEM).
    pub actor_type: String,
    /// Actor identifier (model name, user id, tool name).
    pub actor_id: Option<String>,
    /// Source event id for idempotency (prevents duplicate recording).
    pub source_event_id: Option<String>,
    /// Free-form source adapter name.
    pub source: Option<String>,
    /// Event content (text body, command output, file content, etc.).
    pub content: Option<String>,
    /// Repository name/URL this event relates to.
    pub repository: Option<String>,
    /// Git branch.
    pub branch: Option<String>,
    /// Git commit SHA.
    pub commit_sha: Option<String>,
    /// File path if applicable.
    pub file_path: Option<String>,
    /// Tool name for TOOL_CALL/TOOL_RESULT events.
    pub tool_name: Option<String>,
    /// Tool call id for correlating calls and results.
    pub tool_call_id: Option<String>,
    /// When the event occurred (ISO 8601). Defaults to now.
    pub occurred_at: Option<String>,
    /// Model provider name.
    pub model_provider: Option<String>,
    /// Model name.
    pub model_name: Option<String>,
    /// Security classification. Defaults to INTERNAL.
    pub security_classification: Option<String>,
    /// Opaque metadata (JSON object, bounded).
    pub metadata: Option<serde_json::Value>,
}

#[tool_router]
impl OwnstateMcp {
    pub fn new(services: PrincipalServices, workspace: Option<Workspace>) -> Self {
        Self {
            services,
            workspace,
        }
    }

    #[tool(
        description = "Get one current institutional entity with its exact version and provenance, scoped to the project."
    )]
    pub async fn get_entity(
        &self,
        Parameters(p): Parameters<GetEntityParams>,
    ) -> Result<String, ErrorData> {
        let project_id = parse_id(&p.project_id, "project_id")?;
        let id: ownstate_domain::EntityId = parse_id(&p.entity_id, "entity_id")?;
        let snapshot = self
            .services
            .institutional_snapshot(
                ownstate_services::institutional::InstitutionalQuery {
                    project_id,
                    as_of: None,
                    limit: Some(1),
                },
                Some(id.as_uuid()),
                Some("ENTITY"),
            )
            .await
            .map_err(to_mcp_error)?;
        to_json_text(&snapshot)
    }

    #[tool(
        description = "Get current institutional relationships involving one entity. Includes provenance and temporal validity."
    )]
    pub async fn get_relationships(
        &self,
        Parameters(p): Parameters<GetRelationshipsParams>,
    ) -> Result<String, ErrorData> {
        let project_id = parse_id(&p.project_id, "project_id")?;
        let id: ownstate_domain::EntityId = parse_id(&p.entity_id, "entity_id")?;
        let versions = self
            .services
            .entity_relationships(project_id, id, p.limit.unwrap_or(100), None)
            .await
            .map_err(to_mcp_error)?;
        to_json_text(&versions)
    }

    #[tool(
        description = "Compile a budgeted current context packet for a task and record the exact canonical versions supplied."
    )]
    pub async fn compile_context(
        &self,
        Parameters(p): Parameters<CompileContextParams>,
    ) -> Result<String, ErrorData> {
        let compiled = self
            .services
            .compile_context(ownstate_services::context::CompileRequest {
                project_id: parse_id(&p.project_id, "project_id")?,
                task: p.task,
                session_id: None,
                provider: p.provider,
                model: p.model,
                agent: p.agent,
                max_items: p.max_items,
                token_budget: p.token_budget,
                max_classification: None,
            })
            .await
            .map_err(to_mcp_error)?;
        to_json_text(&compiled)
    }

    /// Resolve which project a tool call addresses. Precedence:
    /// explicit UUID → explicit name → workspace directory name. Name-based
    /// resolution is get-or-create; when the resolved name is the workspace's
    /// own, its git origin (if any) is recorded on the project.
    async fn resolve_project(
        &self,
        project_id: &Option<String>,
        project: &Option<String>,
    ) -> Result<ProjectId, ErrorData> {
        if let Some(raw) = project_id {
            return parse_id(raw, "project_id");
        }
        let workspace_name = self.workspace.as_ref().map(|w| w.name.as_str());
        let name = project
            .as_deref()
            .or(workspace_name)
            .ok_or_else(|| {
                ErrorData::invalid_params(
                    "no project_id or project given, and no workspace directory could be \
                     detected — pass `project` explicitly",
                    None,
                )
            })?
            .to_string();
        let this_workspace = self.workspace.as_ref().filter(|w| w.name == name);
        let git_origin = this_workspace.and_then(|w| w.git_origin.clone());
        let root_path = this_workspace.map(|w| w.root_path.clone());

        let resolved = self
            .services
            .ensure_project(EnsureProject {
                name,
                git_origin,
                root_path,
            })
            .await
            .map_err(to_mcp_error)?;
        Ok(resolved.id)
    }

    #[tool(
        description = "Load a project's current knowledge state: summary counts plus the most \
                       recent ACTIVE knowledge with provenance. Call this once at the start of \
                       a session. With no arguments the project is resolved from the current \
                       workspace directory (created on first use, git origin recorded)."
    )]
    pub async fn bootstrap_project(
        &self,
        Parameters(p): Parameters<BootstrapProjectParams>,
    ) -> Result<String, ErrorData> {
        let project_id = self.resolve_project(&p.project_id, &p.project).await?;
        let bootstrap = self
            .services
            .bootstrap_project(project_id, p.max_items.unwrap_or(20), p.agent)
            .await
            .map_err(to_mcp_error)?;
        to_json_text(&bootstrap)
    }

    #[tool(
        description = "Hybrid search (full-text + semantic) over a project's ACTIVE canonical \
                       knowledge. Returns ranked knowledge with trust levels and evidence ids. \
                       With no project argument the current workspace directory's project is \
                       searched."
    )]
    pub async fn search_knowledge(
        &self,
        Parameters(p): Parameters<SearchKnowledgeParams>,
    ) -> Result<String, ErrorData> {
        let project_id = self.resolve_project(&p.project_id, &p.project).await?;
        let kinds = match p.kinds {
            Some(raw) => {
                let parsed: Result<Vec<KnowledgeKind>, _> =
                    raw.iter().map(|s| s.parse::<KnowledgeKind>()).collect();
                Some(parsed.map_err(|e| ErrorData::invalid_params(e.to_string(), None))?)
            }
            None => None,
        };
        let hits = self
            .services
            .search_knowledge(SearchParams {
                project_id,
                query: p.query,
                kinds,
                limit: p.limit.unwrap_or(10),
                max_classification: SecurityClassification::Confidential,
            })
            .await
            .map_err(to_mcp_error)?;

        let results: Vec<serde_json::Value> = hits
            .iter()
            .map(|h| {
                serde_json::json!({
                    "score": h.score,
                    "knowledge_item_id": h.item.id,
                    "knowledge_version_id": h.version.id,
                    "kind": h.item.kind,
                    "subject_key": h.item.subject_key,
                    "content": h.version.content,
                    "version_number": h.version.version_number,
                    "trust_level": h.version.trust_level,
                    "confidence": h.version.confidence,
                    "evidence_event_ids": h.evidence.iter()
                        .filter_map(|e| e.event_id)
                        .collect::<Vec<_>>(),
                })
            })
            .collect();
        to_json_text(&results)
    }

    #[tool(
        description = "Fetch one knowledge item with its full version history and provenance \
                       (which interaction events each version derives from)."
    )]
    pub async fn get_knowledge(
        &self,
        Parameters(p): Parameters<GetKnowledgeParams>,
    ) -> Result<String, ErrorData> {
        let id: KnowledgeItemId = parse_id(&p.knowledge_item_id, "knowledge_item_id")?;
        let detail = self
            .services
            .get_knowledge(id)
            .await
            .map_err(to_mcp_error)?;
        to_json_text(&serde_json::json!({
            "item": detail.item,
            "versions": detail.versions,
            "evidence": detail.evidence,
        }))
    }

    #[tool(
        description = "Propose new candidate knowledge for a project. The proposal is recorded \
                       as PENDING and reviewed by Ownstate's deterministic promotion rules; it \
                       does NOT directly become trusted project knowledge."
    )]
    pub async fn propose_knowledge(
        &self,
        Parameters(p): Parameters<ProposeKnowledgeParams>,
    ) -> Result<String, ErrorData> {
        let project_id = self.resolve_project(&p.project_id, &p.project).await?;
        let session_id: Option<SessionId> = match &p.session_id {
            Some(raw) => Some(parse_id(raw, "session_id")?),
            None => None,
        };
        let kind: KnowledgeKind = p.kind.parse().map_err(|e: ownstate_domain::DomainError| {
            ErrorData::invalid_params(e.to_string(), None)
        })?;
        let evidence_event_ids: Vec<EventId> = match p.evidence_event_ids {
            Some(raw) => raw
                .iter()
                .map(|s| parse_id(s, "evidence_event_id"))
                .collect::<Result<_, _>>()?,
            None => Vec::new(),
        };

        let candidate = self
            .services
            .propose_knowledge(ownstate_services::knowledge::ProposeKnowledge {
                project_id,
                session_id,
                kind,
                subject_key: p.subject_key,
                content: p.content,
                structured_content: None,
                confidence: p.confidence,
                // MCP callers are AI agents by definition; they can never
                // claim human-explicit trust for their proposals.
                proposed_by: ProposerKind::Agent,
                source: Some("mcp".to_string()),
                evidence_event_ids,
                security_classification: None,
            })
            .await
            .map_err(to_mcp_error)?;

        let candidate_id: CandidateId = candidate.id;
        to_json_text(&serde_json::json!({
            "candidate_id": candidate_id,
            "status": candidate.status,
            "note": "Recorded as candidate knowledge; promotion is decided by Ownstate.",
        }))
    }

    #[tool(
        description = "Record an observable event as append-only evidence. This is the \
                       non-negotiable write path for capture. It does NOT update canonical \
                       truth, promote knowledge, or delete/rewrite existing data. The model \
                       provides observable facts; Ownstate determines derived state."
    )]
    pub async fn record(
        &self,
        Parameters(p): Parameters<RecordParams>,
    ) -> Result<String, ErrorData> {
        let project_id = self.resolve_project(&p.project_id, &p.project).await?;

        let event_type: InteractionEventType =
            p.event_type
                .parse()
                .map_err(|e: ownstate_domain::DomainError| {
                    ErrorData::invalid_params(e.to_string(), None)
                })?;
        let actor_type: ActorType =
            p.actor_type
                .parse()
                .map_err(|e: ownstate_domain::DomainError| {
                    ErrorData::invalid_params(e.to_string(), None)
                })?;
        let security_classification: Option<SecurityClassification> =
            match &p.security_classification {
                Some(raw) => Some(raw.parse().map_err(|e: ownstate_domain::DomainError| {
                    ErrorData::invalid_params(e.to_string(), None)
                })?),
                None => None,
            };
        let occurred_at: Option<chrono::DateTime<chrono::Utc>> = match &p.occurred_at {
            Some(raw) => Some(raw.parse().map_err(|e| {
                ErrorData::invalid_params(format!("invalid occurred_at: {e}"), None)
            })?),
            None => None,
        };

        // Determine session: use existing or create a new one for this record
        let session = self
            .services
            .create_session(ownstate_services::sessions::CreateSession {
                project_id,
                source: "mcp-record".to_string(),
                source_session_id: None,
                agent: p.actor_id.clone(),
                provider: p.model_provider.clone(),
                model: p.model_name.clone(),
                repository: p.repository.clone(),
                initial_commit: p.commit_sha.clone(),
                metadata: None,
            })
            .await
            .map_err(to_mcp_error)?;

        let event = ownstate_domain::NewInteractionEvent {
            event_type,
            actor_type,
            actor_id: p.actor_id,
            sequence: None,
            occurred_at,
            source_event_id: p.source_event_id,
            model_provider: p.model_provider,
            model_name: p.model_name,
            content: p.content,
            tool_name: p.tool_name,
            tool_call_id: p.tool_call_id,
            repository: p.repository,
            branch: p.branch,
            commit_sha: p.commit_sha,
            file_path: p.file_path,
            security_classification,
            metadata: p.metadata,
        };

        let events = self
            .services
            .append_events(session.id, vec![event])
            .await
            .map_err(to_mcp_error)?;

        let event_id = events.first().map(|e| e.id);
        to_json_text(&serde_json::json!({
            "event_id": event_id,
            "session_id": session.id,
            "project_id": project_id,
            "note": "Event appended as raw evidence. Ownstate will process it through the knowledge pipeline.",
        }))
    }
}

#[tool_handler]
impl ServerHandler for OwnstateMcp {
    fn get_info(&self) -> rmcp::model::ServerInfo {
        let mut info = rmcp::model::ServerInfo::default();
        info.capabilities = rmcp::model::ServerCapabilities::builder()
            .enable_tools()
            .build();
        info.instructions = Some(
            "Ownstate is a persistent, model-agnostic knowledge layer. Use \
             bootstrap_project at session start, search_knowledge for task-specific \
             context, get_knowledge for version history and provenance, and \
             propose_knowledge to record new durable learnings as candidates."
                .into(),
        );
        info
    }
}
