//! Database row types and their conversions into domain entities.
//!
//! Rows keep enum columns as `String` so the storage layer stays decoupled
//! from serde representations; conversion into domain types validates every
//! value read from the database.

use std::str::FromStr;

use chrono::{DateTime, Utc};
use ownstate_domain::*;
use serde_json::Value as Json;
use uuid::Uuid;

use crate::error::StorageError;

#[derive(Debug, sqlx::FromRow)]
pub struct ProjectRow {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub ownership_domain: String,
    pub name: String,
    pub description: Option<String>,
    pub metadata: Json,
    pub created_at: DateTime<Utc>,
}

impl TryFrom<ProjectRow> for Project {
    type Error = StorageError;

    fn try_from(r: ProjectRow) -> Result<Self, Self::Error> {
        Ok(Project {
            id: r.id.into(),
            tenant_id: r.tenant_id.into(),
            ownership_domain: OwnershipDomain::from_str(&r.ownership_domain)?,
            name: r.name,
            description: r.description,
            metadata: r.metadata,
            created_at: r.created_at,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct SessionRow {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub project_id: Uuid,
    pub source: String,
    pub source_session_id: Option<String>,
    pub agent: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub status: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub repository: Option<String>,
    pub initial_commit: Option<String>,
    pub final_commit: Option<String>,
    pub metadata: Json,
    pub created_at: DateTime<Utc>,
}

impl TryFrom<SessionRow> for Session {
    type Error = StorageError;

    fn try_from(r: SessionRow) -> Result<Self, Self::Error> {
        Ok(Session {
            id: r.id.into(),
            tenant_id: r.tenant_id.into(),
            project_id: r.project_id.into(),
            source: r.source,
            source_session_id: r.source_session_id,
            agent: r.agent,
            provider: r.provider,
            model: r.model,
            status: SessionStatus::from_str(&r.status)?,
            started_at: r.started_at,
            ended_at: r.ended_at,
            repository: r.repository,
            initial_commit: r.initial_commit,
            final_commit: r.final_commit,
            metadata: r.metadata,
            created_at: r.created_at,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct EventRow {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub project_id: Uuid,
    pub session_id: Uuid,
    pub source: String,
    pub source_event_id: Option<String>,
    pub event_type: String,
    pub actor_type: String,
    pub actor_id: Option<String>,
    pub sequence: i64,
    pub occurred_at: DateTime<Utc>,
    pub model_provider: Option<String>,
    pub model_name: Option<String>,
    pub content: Option<String>,
    pub content_hash: Option<String>,
    pub tool_name: Option<String>,
    pub tool_call_id: Option<String>,
    pub repository: Option<String>,
    pub branch: Option<String>,
    pub commit_sha: Option<String>,
    pub file_path: Option<String>,
    pub security_classification: String,
    pub metadata: Json,
    pub created_at: DateTime<Utc>,
}

impl TryFrom<EventRow> for InteractionEvent {
    type Error = StorageError;

    fn try_from(r: EventRow) -> Result<Self, Self::Error> {
        Ok(InteractionEvent {
            id: r.id.into(),
            tenant_id: r.tenant_id.into(),
            project_id: r.project_id.into(),
            session_id: r.session_id.into(),
            source: r.source,
            source_event_id: r.source_event_id,
            event_type: InteractionEventType::from_str(&r.event_type)?,
            actor_type: ActorType::from_str(&r.actor_type)?,
            actor_id: r.actor_id,
            sequence: r.sequence,
            occurred_at: r.occurred_at,
            model_provider: r.model_provider,
            model_name: r.model_name,
            content: r.content,
            content_hash: r.content_hash,
            tool_name: r.tool_name,
            tool_call_id: r.tool_call_id,
            repository: r.repository,
            branch: r.branch,
            commit_sha: r.commit_sha,
            file_path: r.file_path,
            security_classification: SecurityClassification::from_str(&r.security_classification)?,
            metadata: r.metadata,
            created_at: r.created_at,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct CandidateRow {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub project_id: Uuid,
    pub session_id: Option<Uuid>,
    pub kind: String,
    pub subject_key: String,
    pub content: String,
    pub structured_content: Option<Json>,
    pub confidence: Option<f32>,
    pub proposed_by: String,
    pub source: Option<String>,
    pub evidence_event_ids: Vec<Uuid>,
    pub security_classification: String,
    pub status: String,
    pub rejection_reason: Option<String>,
    pub promoted_version_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TryFrom<CandidateRow> for CandidateKnowledge {
    type Error = StorageError;

    fn try_from(r: CandidateRow) -> Result<Self, Self::Error> {
        Ok(CandidateKnowledge {
            id: r.id.into(),
            tenant_id: r.tenant_id.into(),
            project_id: r.project_id.into(),
            session_id: r.session_id.map(Into::into),
            kind: KnowledgeKind::from_str(&r.kind)?,
            subject_key: r.subject_key,
            content: r.content,
            structured_content: r.structured_content,
            confidence: r.confidence,
            proposed_by: ProposerKind::from_str(&r.proposed_by)?,
            source: r.source,
            evidence_event_ids: r.evidence_event_ids.into_iter().map(Into::into).collect(),
            security_classification: SecurityClassification::from_str(&r.security_classification)?,
            status: CandidateStatus::from_str(&r.status)?,
            rejection_reason: r.rejection_reason,
            promoted_version_id: r.promoted_version_id.map(Into::into),
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct KnowledgeItemRow {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub project_id: Uuid,
    pub ownership_domain: String,
    pub kind: String,
    pub subject_key: String,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
}

impl TryFrom<KnowledgeItemRow> for KnowledgeItem {
    type Error = StorageError;

    fn try_from(r: KnowledgeItemRow) -> Result<Self, Self::Error> {
        Ok(KnowledgeItem {
            id: r.id.into(),
            tenant_id: r.tenant_id.into(),
            project_id: r.project_id.into(),
            ownership_domain: OwnershipDomain::from_str(&r.ownership_domain)?,
            kind: KnowledgeKind::from_str(&r.kind)?,
            subject_key: r.subject_key,
            created_at: r.created_at,
            created_by: r.created_by,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct KnowledgeVersionRow {
    pub id: Uuid,
    pub knowledge_item_id: Uuid,
    pub version_number: i32,
    pub content: String,
    pub structured_content: Option<Json>,
    pub status: String,
    pub trust_level: String,
    pub confidence: Option<f32>,
    pub security_classification: String,
    pub valid_from: DateTime<Utc>,
    pub valid_until: Option<DateTime<Utc>>,
    pub supersedes_version_id: Option<Uuid>,
    pub candidate_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
}

impl TryFrom<KnowledgeVersionRow> for KnowledgeVersion {
    type Error = StorageError;

    fn try_from(r: KnowledgeVersionRow) -> Result<Self, Self::Error> {
        Ok(KnowledgeVersion {
            id: r.id.into(),
            knowledge_item_id: r.knowledge_item_id.into(),
            version_number: r.version_number,
            content: r.content,
            structured_content: r.structured_content,
            status: KnowledgeStatus::from_str(&r.status)?,
            trust_level: TrustLevel::from_str(&r.trust_level)?,
            confidence: r.confidence,
            security_classification: SecurityClassification::from_str(&r.security_classification)?,
            valid_from: r.valid_from,
            valid_until: r.valid_until,
            supersedes_version_id: r.supersedes_version_id.map(Into::into),
            candidate_id: r.candidate_id.map(Into::into),
            created_at: r.created_at,
            created_by: r.created_by,
        })
    }
}

/// Flattened join of a knowledge version with its owning item, with the item
/// columns aliased to avoid name collisions.
#[derive(Debug, sqlx::FromRow)]
pub struct VersionItemJoinRow {
    pub id: Uuid,
    pub knowledge_item_id: Uuid,
    pub version_number: i32,
    pub content: String,
    pub structured_content: Option<Json>,
    pub status: String,
    pub trust_level: String,
    pub confidence: Option<f32>,
    pub security_classification: String,
    pub valid_from: DateTime<Utc>,
    pub valid_until: Option<DateTime<Utc>>,
    pub supersedes_version_id: Option<Uuid>,
    pub candidate_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub item_id: Uuid,
    pub item_tenant_id: Uuid,
    pub item_project_id: Uuid,
    pub item_ownership_domain: String,
    pub item_kind: String,
    pub item_subject_key: String,
    pub item_created_at: DateTime<Utc>,
    pub item_created_by: String,
}

impl TryFrom<VersionItemJoinRow> for (KnowledgeVersion, KnowledgeItem) {
    type Error = StorageError;

    fn try_from(r: VersionItemJoinRow) -> Result<Self, Self::Error> {
        let version = KnowledgeVersion {
            id: r.id.into(),
            knowledge_item_id: r.knowledge_item_id.into(),
            version_number: r.version_number,
            content: r.content,
            structured_content: r.structured_content,
            status: KnowledgeStatus::from_str(&r.status)?,
            trust_level: TrustLevel::from_str(&r.trust_level)?,
            confidence: r.confidence,
            security_classification: SecurityClassification::from_str(&r.security_classification)?,
            valid_from: r.valid_from,
            valid_until: r.valid_until,
            supersedes_version_id: r.supersedes_version_id.map(Into::into),
            candidate_id: r.candidate_id.map(Into::into),
            created_at: r.created_at,
            created_by: r.created_by,
        };
        let item = KnowledgeItem {
            id: r.item_id.into(),
            tenant_id: r.item_tenant_id.into(),
            project_id: r.item_project_id.into(),
            ownership_domain: OwnershipDomain::from_str(&r.item_ownership_domain)?,
            kind: KnowledgeKind::from_str(&r.item_kind)?,
            subject_key: r.item_subject_key,
            created_at: r.item_created_at,
            created_by: r.item_created_by,
        };
        Ok((version, item))
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct EvidenceRow {
    pub id: Uuid,
    pub knowledge_version_id: Uuid,
    pub event_id: Option<Uuid>,
    pub source_type: String,
    pub content_hash: Option<String>,
    pub repository: Option<String>,
    pub commit_sha: Option<String>,
    pub file_path: Option<String>,
    pub line_start: Option<i32>,
    pub line_end: Option<i32>,
    pub created_at: DateTime<Utc>,
}

impl From<EvidenceRow> for KnowledgeEvidence {
    fn from(r: EvidenceRow) -> Self {
        KnowledgeEvidence {
            id: r.id.into(),
            knowledge_version_id: r.knowledge_version_id.into(),
            event_id: r.event_id.map(Into::into),
            source_type: r.source_type,
            content_hash: r.content_hash,
            repository: r.repository,
            commit_sha: r.commit_sha,
            file_path: r.file_path,
            line_start: r.line_start,
            line_end: r.line_end,
            created_at: r.created_at,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct ContextPacketRow {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub project_id: Uuid,
    pub session_id: Option<Uuid>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub task: String,
    pub max_items: i32,
    pub token_budget: Option<i64>,
    pub max_classification: String,
    pub knowledge_version_ids: Vec<Uuid>,
    pub estimated_tokens: i64,
    pub created_at: DateTime<Utc>,
}

impl TryFrom<ContextPacketRow> for ContextPacket {
    type Error = StorageError;

    fn try_from(r: ContextPacketRow) -> Result<Self, Self::Error> {
        Ok(ContextPacket {
            id: r.id.into(),
            tenant_id: r.tenant_id.into(),
            project_id: r.project_id.into(),
            session_id: r.session_id.map(Into::into),
            provider: r.provider,
            model: r.model,
            agent: r.agent,
            task: r.task,
            max_items: r.max_items,
            token_budget: r.token_budget,
            max_classification: SecurityClassification::from_str(&r.max_classification)?,
            knowledge_version_ids: r
                .knowledge_version_ids
                .into_iter()
                .map(Into::into)
                .collect(),
            estimated_tokens: r.estimated_tokens,
            created_at: r.created_at,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct JobRow {
    pub id: Uuid,
    pub kind: String,
    pub payload: Json,
    pub status: String,
    pub attempts: i32,
    pub max_attempts: i32,
    pub run_at: DateTime<Utc>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TryFrom<JobRow> for Job {
    type Error = StorageError;

    fn try_from(r: JobRow) -> Result<Self, Self::Error> {
        Ok(Job {
            id: r.id.into(),
            kind: JobKind::from_str(&r.kind)?,
            payload: r.payload,
            status: JobStatus::from_str(&r.status)?,
            attempts: r.attempts,
            max_attempts: r.max_attempts,
            run_at: r.run_at,
            claimed_at: r.claimed_at,
            finished_at: r.finished_at,
            last_error: r.last_error,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
    }
}
