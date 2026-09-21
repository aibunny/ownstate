//! Project bootstrap: "what does this project currently know", without a
//! task query. Used by the MCP `bootstrap_project` tool at session start.
//! The result is recorded as a context packet like any other context handed
//! to an AI.

use chrono::Utc;
use ownstate_domain::{
    ContextPacket, ContextPacketId, ProjectId, SecurityClassification, estimate_tokens,
};
use ownstate_storage::{candidates, context_packets, knowledge, projects, retrieval};
use serde::Serialize;

use crate::context::{ContextItem, EvidenceRef};
use crate::{AppServices, ServiceResult};

#[derive(Debug, Serialize)]
pub struct ProjectBootstrap {
    pub context_packet_id: String,
    pub project_id: String,
    pub name: String,
    pub description: Option<String>,
    /// Git origin recorded for this project's workspace, when known.
    pub git_origin: Option<String>,
    /// Filesystem location the workspace was last seen at, when known.
    pub root_path: Option<String>,
    pub session_count: i64,
    pub event_count: i64,
    pub active_knowledge_count: i64,
    /// Proposals awaiting promotion. Non-zero with an empty knowledge list
    /// means knowledge has been captured but not yet curated into canonical
    /// state — not that the project is empty.
    pub pending_candidate_count: i64,
    pub knowledge: Vec<ContextItem>,
    pub estimated_tokens: i64,
}

impl AppServices {
    pub(crate) async fn bootstrap_project(
        &self,
        project_id: ProjectId,
        max_items: usize,
        agent: Option<String>,
    ) -> ServiceResult<ProjectBootstrap> {
        let project = projects::get(self.pool(), self.tenant_id(), project_id).await?;
        let (session_count, event_count, active_knowledge_count) =
            projects::counts(self.pool(), self.tenant_id(), project.id).await?;
        let pending_candidate_count =
            candidates::count_pending_for_project(self.pool(), self.tenant_id(), project.id)
                .await?;

        let max_classification = self.max_classification();
        let allowed = SecurityClassification::allowed_up_to(max_classification);
        let ids = retrieval::recent_active(
            self.pool(),
            self.tenant_id(),
            project.id,
            &allowed,
            max_items.clamp(1, ownstate_domain::limits::MAX_CONTEXT_ITEMS) as i64,
        )
        .await?;

        let loaded = knowledge::load_retrievable_versions_with_items(
            self.pool(),
            self.tenant_id(),
            project.id,
            &ids,
            &allowed,
        )
        .await?;
        let loaded_ids: Vec<_> = loaded.iter().map(|(v, _)| v.id).collect();
        let evidence = knowledge::list_evidence_for_versions(self.pool(), &loaded_ids).await?;

        let mut total_tokens = 0i64;
        let mut items = Vec::with_capacity(loaded.len());
        for (version, item) in loaded {
            let tokens = estimate_tokens(&version.content);
            total_tokens += tokens;
            let sources = evidence
                .iter()
                .filter(|e| e.knowledge_version_id == version.id)
                .map(|e| EvidenceRef {
                    evidence_id: e.id.to_string(),
                    event_id: e.event_id.map(|id| id.to_string()),
                    source_type: e.source_type.clone(),
                    content_hash: e.content_hash.clone(),
                    repository: e.repository.clone(),
                    commit_sha: e.commit_sha.clone(),
                    file_path: e.file_path.clone(),
                })
                .collect();
            items.push(ContextItem {
                knowledge_item_id: item.id.to_string(),
                knowledge_version_id: version.id.to_string(),
                kind: item.kind,
                subject_key: item.subject_key,
                content: version.content,
                version_number: version.version_number,
                trust_level: version.trust_level,
                confidence: version.confidence,
                valid_from: version.valid_from,
                estimated_tokens: tokens,
                sources,
            });
        }

        let packet = ContextPacket {
            id: ContextPacketId::generate(),
            tenant_id: self.tenant_id(),
            project_id: project.id,
            session_id: None,
            provider: None,
            model: None,
            agent,
            task: "PROJECT_BOOTSTRAP".to_string(),
            max_items: items.len().max(1) as i32,
            token_budget: None,
            max_classification,
            knowledge_version_ids: items
                .iter()
                .filter_map(|i| i.knowledge_version_id.parse().ok())
                .collect(),
            estimated_tokens: total_tokens,
            created_at: Utc::now(),
        };
        context_packets::insert(self.pool(), &packet).await?;

        Ok(ProjectBootstrap {
            context_packet_id: packet.id.to_string(),
            project_id: project.id.to_string(),
            name: project.name,
            description: project.description,
            git_origin: project
                .metadata
                .get("git_origin")
                .and_then(|v| v.as_str())
                .map(String::from),
            root_path: project
                .metadata
                .get("root_path")
                .and_then(|v| v.as_str())
                .map(String::from),
            session_count,
            event_count,
            active_knowledge_count,
            pending_candidate_count,
            knowledge: items,
            estimated_tokens: total_tokens,
        })
    }
}
