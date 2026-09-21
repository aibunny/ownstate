//! Core domain entities. These mirror the canonical PostgreSQL schema but
//! carry no persistence concerns.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

use crate::enums::*;
use crate::ids::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub tenant_id: TenantId,
    pub ownership_domain: OwnershipDomain,
    pub name: String,
    pub description: Option<String>,
    pub metadata: Json,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub tenant_id: TenantId,
    pub project_id: ProjectId,
    /// Which adapter/provider produced this session ("claude-code", "codex",
    /// "chat-import", "manual", ...). Free-form; adapters normalize into it.
    pub source: String,
    pub source_session_id: Option<String>,
    pub agent: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub status: SessionStatus,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub repository: Option<String>,
    pub initial_commit: Option<String>,
    pub final_commit: Option<String>,
    pub metadata: Json,
    pub created_at: DateTime<Utc>,
}

/// A single normalized, append-only interaction event (raw evidence).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionEvent {
    pub id: EventId,
    pub tenant_id: TenantId,
    pub project_id: ProjectId,
    pub session_id: SessionId,
    pub source: String,
    pub source_event_id: Option<String>,
    pub event_type: InteractionEventType,
    pub actor_type: ActorType,
    pub actor_id: Option<String>,
    /// Strictly ordered position within the session.
    pub sequence: i64,
    pub occurred_at: DateTime<Utc>,
    pub model_provider: Option<String>,
    pub model_name: Option<String>,
    pub content: Option<String>,
    /// BLAKE3 digest of `content`, computed server-side at append time.
    pub content_hash: Option<String>,
    pub tool_name: Option<String>,
    pub tool_call_id: Option<String>,
    pub repository: Option<String>,
    pub branch: Option<String>,
    pub commit_sha: Option<String>,
    pub file_path: Option<String>,
    pub security_classification: SecurityClassification,
    pub metadata: Json,
    pub created_at: DateTime<Utc>,
}

/// Client-supplied payload for appending one event. Ids, hashes and tenant
/// scope are assigned server-side.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewInteractionEvent {
    pub event_type: InteractionEventType,
    pub actor_type: ActorType,
    pub actor_id: Option<String>,
    /// Optional explicit sequence. When omitted the next free sequence for
    /// the session is assigned.
    pub sequence: Option<i64>,
    pub occurred_at: Option<DateTime<Utc>>,
    pub source_event_id: Option<String>,
    pub model_provider: Option<String>,
    pub model_name: Option<String>,
    pub content: Option<String>,
    pub tool_name: Option<String>,
    pub tool_call_id: Option<String>,
    pub repository: Option<String>,
    pub branch: Option<String>,
    pub commit_sha: Option<String>,
    pub file_path: Option<String>,
    pub security_classification: Option<SecurityClassification>,
    pub metadata: Option<Json>,
}

/// Untrusted proposal layer: something that may deserve to become canonical.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateKnowledge {
    pub id: CandidateId,
    pub tenant_id: TenantId,
    pub project_id: ProjectId,
    pub session_id: Option<SessionId>,
    pub kind: KnowledgeKind,
    pub subject_key: String,
    pub content: String,
    pub structured_content: Option<Json>,
    pub confidence: Option<f32>,
    pub proposed_by: ProposerKind,
    /// Free-form descriptor of where the proposal came from
    /// ("mcp:claude-code", "api", "import:chatgpt", ...).
    pub source: Option<String>,
    pub evidence_event_ids: Vec<EventId>,
    pub security_classification: SecurityClassification,
    pub status: CandidateStatus,
    pub rejection_reason: Option<String>,
    pub promoted_version_id: Option<KnowledgeVersionId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Durable conceptual identity of a piece of knowledge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeItem {
    pub id: KnowledgeItemId,
    pub tenant_id: TenantId,
    pub project_id: ProjectId,
    pub ownership_domain: OwnershipDomain,
    pub kind: KnowledgeKind,
    pub subject_key: String,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
}

/// One immutable version of a knowledge item. Status and temporal validity
/// may change; content never does.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeVersion {
    pub id: KnowledgeVersionId,
    pub knowledge_item_id: KnowledgeItemId,
    pub version_number: i32,
    pub content: String,
    pub structured_content: Option<Json>,
    pub status: KnowledgeStatus,
    pub trust_level: TrustLevel,
    pub confidence: Option<f32>,
    pub security_classification: SecurityClassification,
    pub valid_from: DateTime<Utc>,
    pub valid_until: Option<DateTime<Utc>>,
    pub supersedes_version_id: Option<KnowledgeVersionId>,
    pub candidate_id: Option<CandidateId>,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
}

/// Provenance link from a knowledge version to its evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEvidence {
    pub id: EvidenceId,
    pub knowledge_version_id: KnowledgeVersionId,
    pub event_id: Option<EventId>,
    pub source_type: String,
    pub content_hash: Option<String>,
    pub repository: Option<String>,
    pub commit_sha: Option<String>,
    pub file_path: Option<String>,
    pub line_start: Option<i32>,
    pub line_end: Option<i32>,
    pub created_at: DateTime<Utc>,
}

/// Audit record of exactly which knowledge versions were handed to an AI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPacket {
    pub id: ContextPacketId,
    pub tenant_id: TenantId,
    pub project_id: ProjectId,
    pub session_id: Option<SessionId>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub task: String,
    pub max_items: i32,
    pub token_budget: Option<i64>,
    pub max_classification: SecurityClassification,
    pub knowledge_version_ids: Vec<KnowledgeVersionId>,
    pub estimated_tokens: i64,
    pub created_at: DateTime<Utc>,
}

/// Durable background job (PostgreSQL-backed queue).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: JobId,
    pub kind: JobKind,
    pub payload: Json,
    pub status: JobStatus,
    pub attempts: i32,
    pub max_attempts: i32,
    pub run_at: DateTime<Utc>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
