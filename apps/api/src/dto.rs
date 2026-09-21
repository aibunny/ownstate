//! Request/response DTOs. Domain entities serialize cleanly (vendor-neutral
//! shapes), so responses reuse them; requests get explicit structs.

use ownstate_domain::{
    CandidateId, EventId, KnowledgeEvidence, KnowledgeItem, KnowledgeKind, KnowledgeVersion,
    NewInteractionEvent, OwnershipDomain, ProjectId, ProposerKind, SecurityClassification,
    SessionId,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateProjectRequest {
    pub name: String,
    pub description: Option<String>,
    pub ownership_domain: Option<OwnershipDomain>,
    pub metadata: Option<Json>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateSessionRequest {
    pub project_id: ProjectId,
    pub source: String,
    pub source_session_id: Option<String>,
    pub agent: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub repository: Option<String>,
    pub initial_commit: Option<String>,
    pub metadata: Option<Json>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppendEventsRequest {
    pub events: Vec<NewInteractionEvent>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposeKnowledgeRequest {
    pub project_id: ProjectId,
    pub session_id: Option<SessionId>,
    pub kind: KnowledgeKind,
    pub subject_key: String,
    pub content: String,
    pub structured_content: Option<Json>,
    pub confidence: Option<f32>,
    pub proposed_by: ProposerKind,
    pub source: Option<String>,
    #[serde(default)]
    pub evidence_event_ids: Vec<EventId>,
    pub security_classification: Option<SecurityClassification>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RejectCandidateRequest {
    pub reason: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchQuery {
    pub project_id: ProjectId,
    pub q: String,
    /// Comma-separated knowledge kinds, e.g. "ARCHITECTURE,DECISION".
    pub kinds: Option<String>,
    pub limit: Option<usize>,
    pub max_classification: Option<SecurityClassification>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompileContextRequest {
    pub project_id: ProjectId,
    pub task: String,
    pub session_id: Option<SessionId>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub max_items: Option<usize>,
    pub token_budget: Option<i64>,
    pub max_classification: Option<SecurityClassification>,
}

#[derive(Serialize)]
pub struct PromotionResponse {
    pub item: KnowledgeItem,
    pub version: KnowledgeVersion,
}

#[derive(Serialize)]
pub struct SearchHitResponse {
    pub score: f64,
    pub item: KnowledgeItem,
    pub version: KnowledgeVersion,
    pub evidence: Vec<KnowledgeEvidence>,
}

#[derive(Serialize)]
pub struct KnowledgeDetailResponse {
    pub item: KnowledgeItem,
    pub versions: Vec<KnowledgeVersion>,
    pub evidence: Vec<KnowledgeEvidence>,
}

#[derive(Serialize)]
pub struct CandidateCreatedResponse {
    pub id: CandidateId,
    pub status: String,
    pub project_id: ProjectId,
}
