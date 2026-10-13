use ownstate_domain::{
    ActorType, CandidateId, EventId, InteractionEventType, KnowledgeItemId, KnowledgeKind,
    ProjectId, ProposerKind, SecurityClassification, SessionId,
};
use ownstate_services::policy::{AuthenticatedPrincipal, PrincipalServices};
use ownstate_services::projects::EnsureProject;
use ownstate_services::search::SearchParams;
use ownstate_services::{AppServices, ServiceError};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer, ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::workspace::Workspace;

#[derive(Clone)]
pub struct OwnstateMcp {
    services: McpServices,
    /// Detected at startup from the directory the MCP client launched us in.
    workspace: Option<Workspace>,
}

#[derive(Clone)]
enum McpServices {
    Fixed(PrincipalServices),
    AuthenticatedHttp(std::sync::Arc<AppServices>),
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
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecordConversationParams {
    /// Client name, for example codex, claude, or opencode.
    pub source: String,
    /// Stable host conversation identifier. Reuse this for each message in the chat.
    pub source_conversation_id: String,
    pub title: Option<String>,
    /// Stable host event identifier, if available, for safe retries.
    pub source_event_id: Option<String>,
    /// USER_MESSAGE, ASSISTANT_MESSAGE, TOOL_CALL, TOOL_RESULT, SYSTEM_EVENT, or ARTIFACT_CREATED.
    pub event_type: String,
    /// USER, ASSISTANT, TOOL, or SYSTEM.
    pub actor_type: String,
    pub content: Option<String>,
    pub occurred_at: Option<String>,
    pub security_classification: Option<String>,
}

#[tool_router]
impl OwnstateMcp {
    #[tool(
        description = "Identify the account and space represented by this authenticated Ownstate connection. The opaque id stays the same across token refresh and reconnection.",
        input_schema = std::sync::Arc::new(serde_json::Map::from_iter([
            ("type".to_string(), serde_json::json!("object")),
            ("properties".to_string(), serde_json::json!({})),
            ("additionalProperties".to_string(), serde_json::json!(false)),
        ])),
        output_schema = std::sync::Arc::new(serde_json::Map::from_iter([
            ("$schema".to_string(), serde_json::json!("https://json-schema.org/draft/2020-12/schema")),
            ("type".to_string(), serde_json::json!("object")),
            ("properties".to_string(), serde_json::json!({"id":{"type":"string","minLength":1,"pattern":r"\S"},"email":{"type":"string"},"nickname":{"type":"string"}})),
            ("required".to_string(), serde_json::json!(["id"])),
            ("additionalProperties".to_string(), serde_json::json!(false)),
        ])),
        annotations(title = "Identify Ownstate Connection", read_only_hint = true, destructive_hint = false, open_world_hint = false),
        meta = rmcp::model::MetaObject(serde_json::Map::from_iter([("openai/profile".to_string(), serde_json::Value::Bool(true))]))
    )]
    pub async fn get_profile(
        &self,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let services = self.request_services(&ctx)?;
        let profile = services.connection_profile().await.map_err(to_mcp_error)?;
        let value = serde_json::to_value(profile).map_err(|e| {
            ErrorData::internal_error(format!("profile serialization failed: {e}"), None)
        })?;
        Ok(CallToolResult::structured(value))
    }

    #[tool(
        description = "Append one observable event to a non-project chat using a stable conversation ID and event ID. MCP calls capture only content the client sends through this tool; they do not expose an entire chat transcript automatically.",
        annotations(
            title = "Record Chat Event",
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    pub async fn record_conversation(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<RecordConversationParams>,
    ) -> Result<String, ErrorData> {
        let services = self.request_services(&ctx)?;
        let occurred_at = p
            .occurred_at
            .as_deref()
            .map(str::parse)
            .transpose()
            .map_err(|_| ErrorData::invalid_params("invalid occurred_at", None))?;
        let result = services
            .capture_conversation(ownstate_services::conversations::CaptureConversation {
                source: p.source,
                source_conversation_id: p.source_conversation_id,
                title: p.title,
                capture_mode: "MCP_ONLY".into(),
                source_event_id: p.source_event_id,
                event_type: p.event_type,
                actor_type: p.actor_type,
                content: p.content,
                occurred_at,
                security_classification: p.security_classification,
            })
            .await
            .map_err(to_mcp_error)?;
        to_json_text(&result)
    }

    pub fn new(services: PrincipalServices, workspace: Option<Workspace>) -> Self {
        Self {
            services: McpServices::Fixed(services),
            workspace,
        }
    }

    pub fn authenticated_http(services: std::sync::Arc<AppServices>) -> Self {
        Self {
            services: McpServices::AuthenticatedHttp(services),
            workspace: None,
        }
    }

    fn request_services(
        &self,
        ctx: &RequestContext<RoleServer>,
    ) -> Result<PrincipalServices, ErrorData> {
        match &self.services {
            McpServices::Fixed(scoped) => Ok(scoped.clone()),
            McpServices::AuthenticatedHttp(base) => {
                let principal = ctx
                    .extensions
                    .get::<http::request::Parts>()
                    .and_then(|parts| parts.extensions.get::<AuthenticatedPrincipal>())
                    .ok_or_else(|| {
                        ErrorData::internal_error("authenticated principal missing", None)
                    })?;
                std::sync::Arc::new(base.for_tenant(principal.tenant_id()))
                    .for_principal(principal.clone())
                    .map_err(to_mcp_error)
            }
        }
    }

    #[tool(
        description = "Get one current institutional entity with its exact version and provenance, scoped to the project.",
        annotations(
            title = "Get Institutional Entity",
            read_only_hint = true,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    pub async fn get_entity(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<GetEntityParams>,
    ) -> Result<String, ErrorData> {
        let services = self.request_services(&ctx)?;
        let project_id = parse_id(&p.project_id, "project_id")?;
        let id: ownstate_domain::EntityId = parse_id(&p.entity_id, "entity_id")?;
        let snapshot = services
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
        description = "Get current institutional relationships involving one entity. Includes provenance and temporal validity.",
        annotations(
            title = "Get Entity Relationships",
            read_only_hint = true,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    pub async fn get_relationships(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<GetRelationshipsParams>,
    ) -> Result<String, ErrorData> {
        let services = self.request_services(&ctx)?;
        let project_id = parse_id(&p.project_id, "project_id")?;
        let id: ownstate_domain::EntityId = parse_id(&p.entity_id, "entity_id")?;
        let versions = services
            .entity_relationships(project_id, id, p.limit.unwrap_or(100), None)
            .await
            .map_err(to_mcp_error)?;
        to_json_text(&versions)
    }

    #[tool(
        description = "Compile a budgeted current context packet for a task and record the exact canonical versions supplied.",
        annotations(
            title = "Compile Project Context",
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    pub async fn compile_context(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<CompileContextParams>,
    ) -> Result<String, ErrorData> {
        let services = self.request_services(&ctx)?;
        let compiled = services
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
        services: &PrincipalServices,
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

        let resolved = services
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
                       recent ACTIVE knowledge with provenance. With no arguments the project \
                       is resolved from the current workspace directory (created on first use, \
                       git origin recorded).",
        annotations(
            title = "Load Project Context",
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    pub async fn bootstrap_project(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<BootstrapProjectParams>,
    ) -> Result<String, ErrorData> {
        let services = self.request_services(&ctx)?;
        let project_id = self
            .resolve_project(&services, &p.project_id, &p.project)
            .await?;
        let bootstrap = services
            .bootstrap_project(project_id, p.max_items.unwrap_or(20), p.agent)
            .await
            .map_err(to_mcp_error)?;
        to_json_text(&bootstrap)
    }

    #[tool(
        description = "Hybrid search (full-text + semantic) over a project's ACTIVE canonical \
                       knowledge. Returns ranked knowledge with trust levels and evidence ids. \
                       With no project argument the current workspace directory's project is \
                       searched.",
        annotations(
            title = "Search Project Knowledge",
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    pub async fn search_knowledge(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<SearchKnowledgeParams>,
    ) -> Result<String, ErrorData> {
        let services = self.request_services(&ctx)?;
        let project_id = self
            .resolve_project(&services, &p.project_id, &p.project)
            .await?;
        let kinds = match p.kinds {
            Some(raw) => {
                let parsed: Result<Vec<KnowledgeKind>, _> =
                    raw.iter().map(|s| s.parse::<KnowledgeKind>()).collect();
                Some(parsed.map_err(|e| ErrorData::invalid_params(e.to_string(), None))?)
            }
            None => None,
        };
        let hits = services
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
                       (which interaction events each version derives from).",
        annotations(
            title = "Get Knowledge History",
            read_only_hint = true,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    pub async fn get_knowledge(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<GetKnowledgeParams>,
    ) -> Result<String, ErrorData> {
        let services = self.request_services(&ctx)?;
        let id: KnowledgeItemId = parse_id(&p.knowledge_item_id, "knowledge_item_id")?;
        let detail = services.get_knowledge(id).await.map_err(to_mcp_error)?;
        to_json_text(&serde_json::json!({
            "item": detail.item,
            "versions": detail.versions,
            "evidence": detail.evidence,
        }))
    }

    #[tool(
        description = "Propose new candidate knowledge for a project. The proposal is recorded \
                       as PENDING and reviewed by Ownstate's deterministic promotion rules; it \
                       does NOT directly become trusted project knowledge.",
        annotations(
            title = "Propose Project Knowledge",
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    pub async fn propose_knowledge(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<ProposeKnowledgeParams>,
    ) -> Result<String, ErrorData> {
        let services = self.request_services(&ctx)?;
        let project_id = self
            .resolve_project(&services, &p.project_id, &p.project)
            .await?;
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

        let candidate = services
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
        description = "Record an observable event as append-only project evidence. This tool \
                       does not update canonical knowledge, promote proposals, or delete or \
                       rewrite existing data.",
        annotations(
            title = "Record Project Event",
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    pub async fn record(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(p): Parameters<RecordParams>,
    ) -> Result<String, ErrorData> {
        let services = self.request_services(&ctx)?;
        let project_id = self
            .resolve_project(&services, &p.project_id, &p.project)
            .await?;

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
        let session = services
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
            metadata: p.metadata.map(serde_json::Value::Object),
        };

        let events = services
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
            "Ownstate stores authorized project context, evidence, and proposed \
             knowledge. Its tools retrieve current knowledge and provenance, \
             compile context, and record explicitly submitted events or proposals."
                .into(),
        );
        info
    }
}
