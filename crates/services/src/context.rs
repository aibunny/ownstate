//! The Context Compiler: turn a task into the smallest useful trusted
//! context package, deterministically — no generative model involved.
//!
//! Pipeline: authorization → hybrid retrieval over ACTIVE knowledge →
//! trust-weighted ranking → item/token budgeting → packet audit record.

use chrono::{DateTime, Utc};
use ownstate_domain::{
    ContextPacket, ContextPacketId, KnowledgeKind, ProjectId, SecurityClassification, SessionId,
    TrustLevel, estimate_tokens, limits,
};
use ownstate_storage::{context_packets, sessions};
use serde::Serialize;

use crate::search::SearchParams;
use crate::{AppServices, ServiceError, ServiceResult};

pub struct CompileRequest {
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

#[derive(Debug, Serialize)]
pub struct EvidenceRef {
    pub evidence_id: String,
    pub event_id: Option<String>,
    pub source_type: String,
    pub content_hash: Option<String>,
    pub repository: Option<String>,
    pub commit_sha: Option<String>,
    pub file_path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ContextItem {
    pub knowledge_item_id: String,
    pub knowledge_version_id: String,
    pub kind: KnowledgeKind,
    pub subject_key: String,
    pub content: String,
    pub version_number: i32,
    pub trust_level: TrustLevel,
    pub confidence: Option<f32>,
    pub valid_from: DateTime<Utc>,
    pub estimated_tokens: i64,
    pub sources: Vec<EvidenceRef>,
}

#[derive(Debug, Serialize)]
pub struct ContextSection {
    pub category: &'static str,
    pub items: Vec<ContextItem>,
}

#[derive(Debug, Serialize)]
pub struct CompiledContext {
    pub context_packet_id: String,
    pub project_id: String,
    pub task: String,
    pub sections: Vec<ContextSection>,
    pub estimated_tokens: i64,
    pub max_classification: SecurityClassification,
    pub created_at: DateTime<Utc>,
}

/// Context categories from the requirements, keyed by knowledge kind. Only
/// categories with retrieved content appear in a packet.
const fn category_for(kind: KnowledgeKind) -> &'static str {
    match kind {
        KnowledgeKind::Architecture => "RELEVANT ARCHITECTURE",
        KnowledgeKind::Decision => "RELEVANT DECISIONS",
        KnowledgeKind::Rationale => "RATIONALE",
        KnowledgeKind::Constraint => "CONSTRAINTS",
        KnowledgeKind::Requirement => "REQUIREMENTS",
        KnowledgeKind::Failure => "PREVIOUS FAILURES",
        KnowledgeKind::Outcome => "PREVIOUS OUTCOMES",
        KnowledgeKind::OpenQuestion => "OPEN QUESTIONS",
        KnowledgeKind::Fact | KnowledgeKind::Definition => "FACTS AND DEFINITIONS",
        KnowledgeKind::Procedure => "PROCEDURES",
        KnowledgeKind::Preference => "PREFERENCES",
        KnowledgeKind::Goal => "GOALS",
        KnowledgeKind::Risk => "RISKS",
        KnowledgeKind::Relationship => "RELATIONSHIPS",
    }
}

/// Fixed category order so packets render stably.
const CATEGORY_ORDER: &[&str] = &[
    "RELEVANT ARCHITECTURE",
    "RELEVANT DECISIONS",
    "RATIONALE",
    "CONSTRAINTS",
    "REQUIREMENTS",
    "PREVIOUS FAILURES",
    "PREVIOUS OUTCOMES",
    "GOALS",
    "RISKS",
    "PROCEDURES",
    "PREFERENCES",
    "FACTS AND DEFINITIONS",
    "RELATIONSHIPS",
    "OPEN QUESTIONS",
];

impl AppServices {
    pub(crate) async fn compile_context(
        &self,
        req: CompileRequest,
    ) -> ServiceResult<CompiledContext> {
        let task = req.task.trim().to_string();
        if task.is_empty() || task.len() > limits::MAX_TASK_LEN {
            return Err(ServiceError::validation(format!(
                "task must be 1..={} characters",
                limits::MAX_TASK_LEN
            )));
        }
        let max_items = req.max_items.unwrap_or(20);
        if max_items == 0 || max_items > limits::MAX_CONTEXT_ITEMS {
            return Err(ServiceError::validation(format!(
                "max_items must be 1..={}",
                limits::MAX_CONTEXT_ITEMS
            )));
        }
        if let Some(budget) = req.token_budget
            && budget <= 0
        {
            return Err(ServiceError::validation("token_budget must be positive"));
        }
        // Caller-requested ceilings can only narrow the server-side maximum.
        let max_classification = self.clamp_classification(req.max_classification);

        // Session, when provided, must belong to the project (and tenant).
        if let Some(session_id) = req.session_id {
            let session = sessions::get(self.pool(), self.tenant_id(), session_id).await?;
            if session.project_id != req.project_id {
                return Err(ServiceError::validation(
                    "session does not belong to the project",
                ));
            }
        }

        // Retrieval (project existence — the authorization gate — is checked
        // inside search_knowledge before any data is read). Overfetch so the
        // budget pass has room to choose.
        let hits = self
            .search_knowledge(SearchParams {
                project_id: req.project_id,
                query: task.clone(),
                kinds: None,
                limit: (max_items * 2).min(limits::MAX_CONTEXT_ITEMS),
                max_classification,
            })
            .await?;

        // Budgeting: ranked greedy fill under max_items and token_budget.
        let mut selected = Vec::new();
        let mut total_tokens: i64 = 0;
        for hit in hits {
            if selected.len() >= max_items {
                break;
            }
            let item_tokens = estimate_tokens(&hit.version.content);
            if let Some(budget) = req.token_budget
                && total_tokens + item_tokens > budget
            {
                continue; // try a smaller lower-ranked item before giving up
            }
            total_tokens += item_tokens;
            selected.push((hit, item_tokens));
        }

        let packet = ContextPacket {
            id: ContextPacketId::generate(),
            tenant_id: self.tenant_id(),
            project_id: req.project_id,
            session_id: req.session_id,
            provider: req.provider,
            model: req.model,
            agent: req.agent,
            task: task.clone(),
            max_items: max_items as i32,
            token_budget: req.token_budget,
            max_classification,
            knowledge_version_ids: selected.iter().map(|(h, _)| h.version.id).collect(),
            estimated_tokens: total_tokens,
            created_at: Utc::now(),
        };
        context_packets::insert(self.pool(), &packet).await?;

        // Assemble sections in stable category order.
        let mut sections: Vec<ContextSection> = Vec::new();
        for (hit, item_tokens) in selected {
            let category = category_for(hit.item.kind);
            let context_item = ContextItem {
                knowledge_item_id: hit.item.id.to_string(),
                knowledge_version_id: hit.version.id.to_string(),
                kind: hit.item.kind,
                subject_key: hit.item.subject_key.clone(),
                content: hit.version.content.clone(),
                version_number: hit.version.version_number,
                trust_level: hit.version.trust_level,
                confidence: hit.version.confidence,
                valid_from: hit.version.valid_from,
                estimated_tokens: item_tokens,
                sources: hit
                    .evidence
                    .iter()
                    .map(|e| EvidenceRef {
                        evidence_id: e.id.to_string(),
                        event_id: e.event_id.map(|id| id.to_string()),
                        source_type: e.source_type.clone(),
                        content_hash: e.content_hash.clone(),
                        repository: e.repository.clone(),
                        commit_sha: e.commit_sha.clone(),
                        file_path: e.file_path.clone(),
                    })
                    .collect(),
            };
            match sections.iter_mut().find(|s| s.category == category) {
                Some(section) => section.items.push(context_item),
                None => sections.push(ContextSection {
                    category,
                    items: vec![context_item],
                }),
            }
        }
        sections.sort_by_key(|s| {
            CATEGORY_ORDER
                .iter()
                .position(|c| *c == s.category)
                .unwrap_or(usize::MAX)
        });

        tracing::info!(
            context_packet_id = %packet.id,
            project_id = %packet.project_id,
            items = packet.knowledge_version_ids.len(),
            estimated_tokens = packet.estimated_tokens,
            "context packet compiled"
        );

        Ok(CompiledContext {
            context_packet_id: packet.id.to_string(),
            project_id: packet.project_id.to_string(),
            task,
            sections,
            estimated_tokens: total_tokens,
            max_classification,
            created_at: packet.created_at,
        })
    }
}
