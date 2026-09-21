use chrono::Utc;
use ownstate_domain::{
    InteractionEvent, NewInteractionEvent, ProjectId, SecurityClassification, Session, SessionId,
    SessionStatus, content_hash, limits,
};
use ownstate_storage::{events, projects, sessions};
use serde_json::Value as Json;

use crate::{AppServices, ServiceError, ServiceResult};

pub struct CreateSession {
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

impl AppServices {
    pub(crate) async fn create_session(&self, req: CreateSession) -> ServiceResult<Session> {
        let source = req.source.trim().to_string();
        if source.is_empty() || source.len() > 100 {
            return Err(ServiceError::validation(
                "source must be 1..=100 characters",
            ));
        }
        // Authorization scope: the project must exist within our tenant.
        let project = projects::get(self.pool(), self.tenant_id(), req.project_id).await?;

        let session = Session {
            id: SessionId::generate(),
            tenant_id: self.tenant_id(),
            project_id: project.id,
            source,
            source_session_id: req.source_session_id,
            agent: req.agent,
            provider: req.provider,
            model: req.model,
            status: SessionStatus::Active,
            started_at: Utc::now(),
            ended_at: None,
            repository: req.repository,
            initial_commit: req.initial_commit,
            final_commit: None,
            metadata: req
                .metadata
                .unwrap_or_else(|| Json::Object(Default::default())),
            created_at: Utc::now(),
        };
        sessions::insert(self.pool(), &session).await?;
        tracing::info!(session_id = %session.id, project_id = %project.id, "session created");
        Ok(session)
    }

    pub(crate) async fn get_session(&self, id: SessionId) -> ServiceResult<Session> {
        Ok(sessions::get(self.pool(), self.tenant_id(), id).await?)
    }

    /// Append a batch of normalized events to a session. Events are raw
    /// evidence: validated for size and shape, hashed, and inserted append-only
    /// in one transaction. Sequences are assigned under a session row lock so
    /// concurrent appends cannot interleave.
    pub(crate) async fn append_events(
        &self,
        session_id: SessionId,
        new_events: Vec<NewInteractionEvent>,
    ) -> ServiceResult<Vec<InteractionEvent>> {
        if new_events.is_empty() {
            return Err(ServiceError::validation("event batch is empty"));
        }
        if new_events.len() > limits::MAX_EVENTS_PER_BATCH {
            return Err(ServiceError::validation(format!(
                "event batch exceeds {} events",
                limits::MAX_EVENTS_PER_BATCH
            )));
        }
        for (i, e) in new_events.iter().enumerate() {
            if let Some(content) = &e.content
                && content.len() > limits::MAX_CONTENT_BYTES
            {
                return Err(ServiceError::validation(format!(
                    "event {i} content exceeds {} bytes; store large content in object storage",
                    limits::MAX_CONTENT_BYTES
                )));
            }
            if let Some(seq) = e.sequence
                && seq < 0
            {
                return Err(ServiceError::validation(format!(
                    "event {i} sequence must be >= 0"
                )));
            }
        }

        let mut tx = self.pool().begin().await?;
        let session = sessions::lock_for_append(&mut tx, self.tenant_id(), session_id).await?;
        let mut next_sequence = events::max_sequence(&mut tx, session.id)
            .await?
            .checked_add(1)
            .ok_or_else(|| ServiceError::validation("session sequence space is exhausted"))?;

        let mut inserted = Vec::with_capacity(new_events.len());
        for e in new_events {
            // Explicit sequences may only jump a bounded distance ahead, so a
            // single crafted value cannot exhaust the sequence space and
            // poison every future append to this session.
            let sequence = match e.sequence {
                Some(seq) => {
                    if seq > next_sequence.saturating_add(limits::MAX_SEQUENCE_GAP) {
                        return Err(ServiceError::validation(format!(
                            "explicit sequence {seq} is more than {} ahead of the next \
                             expected sequence {next_sequence}",
                            limits::MAX_SEQUENCE_GAP
                        )));
                    }
                    seq
                }
                None => next_sequence,
            };
            next_sequence = next_sequence.max(sequence.checked_add(1).ok_or_else(|| {
                ServiceError::validation("event sequence overflows the sequence space")
            })?);

            let event = InteractionEvent {
                id: ownstate_domain::EventId::generate(),
                tenant_id: session.tenant_id,
                project_id: session.project_id,
                session_id: session.id,
                source: session.source.clone(),
                source_event_id: e.source_event_id,
                event_type: e.event_type,
                actor_type: e.actor_type,
                actor_id: e.actor_id,
                sequence,
                occurred_at: e.occurred_at.unwrap_or_else(Utc::now),
                model_provider: e.model_provider,
                model_name: e.model_name,
                content_hash: e.content.as_deref().map(|c| content_hash(c.as_bytes())),
                content: e.content,
                tool_name: e.tool_name,
                tool_call_id: e.tool_call_id,
                repository: e.repository,
                branch: e.branch,
                commit_sha: e.commit_sha,
                file_path: e.file_path,
                security_classification: e
                    .security_classification
                    .unwrap_or(SecurityClassification::Internal),
                metadata: e
                    .metadata
                    .unwrap_or_else(|| Json::Object(Default::default())),
                created_at: Utc::now(),
            };
            events::insert(&mut *tx, &event).await?;
            inserted.push(event);
        }
        tx.commit().await?;

        tracing::info!(
            session_id = %session_id,
            count = inserted.len(),
            "events appended"
        );
        Ok(inserted)
    }

    pub(crate) async fn list_events(
        &self,
        session_id: SessionId,
    ) -> ServiceResult<Vec<InteractionEvent>> {
        // Scope check: session must exist in-tenant before events are read.
        let session = sessions::get(self.pool(), self.tenant_id(), session_id).await?;
        let ceiling = self.max_classification();
        // The classification ceiling applies to raw evidence too: events above
        // it stay visible as rows (sequence continuity and hashes are audit
        // data) but their content is withheld, same policy as knowledge reads.
        Ok(events::list_for_session(self.pool(), session.id)
            .await?
            .into_iter()
            .map(|mut e| {
                if e.security_classification > ceiling {
                    e.content = e
                        .content
                        .map(|_| format!("[content withheld: {}]", e.security_classification));
                    e.metadata = Json::Object(Default::default());
                }
                e
            })
            .collect())
    }
}
