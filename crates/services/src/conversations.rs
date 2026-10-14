//! Append-only capture for chats that do not belong to a coding project.
use chrono::{DateTime, Utc};
use ownstate_domain::{ExecutionBudget, PolicyAction, ResourceScope, SecurityClassification};
use ownstate_storage::policy;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::policy::PrincipalServices;
use crate::{ServiceError, ServiceResult};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureConversation {
    pub source: String,
    pub source_conversation_id: String,
    pub title: Option<String>,
    pub capture_mode: String,
    pub source_event_id: Option<String>,
    pub event_type: String,
    pub actor_type: String,
    pub content: Option<String>,
    pub occurred_at: Option<DateTime<Utc>>,
    pub security_classification: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CapturedEvent {
    pub thread_id: Uuid,
    pub event_id: Uuid,
    pub sequence: i64,
    pub duplicate: bool,
}

#[derive(Debug, Serialize)]
pub struct ConversationSummary {
    pub id: Uuid,
    pub source: String,
    pub source_conversation_id: String,
    pub title: Option<String>,
    pub capture_mode: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ConversationEvent {
    pub id: Uuid,
    pub sequence: i64,
    pub event_type: String,
    pub actor_type: String,
    pub content: Option<String>,
    pub occurred_at: DateTime<Utc>,
}

fn valid_enum(value: &str, allowed: &[&str]) -> bool {
    allowed.contains(&value)
}

impl PrincipalServices {
    async fn conversation_workspace(
        &self,
        action: PolicyAction,
    ) -> ServiceResult<(Uuid, SecurityClassification)> {
        let app = self.app();
        let (workspace_id, _) =
            policy::project_workspace_for_principal(app.pool(), self.principal())
                .await?
                .ok_or_else(|| ServiceError::forbidden("principal has no active workspace"))?;
        let scope = self
            .authorize(
                action,
                ResourceScope {
                    tenant_id: self.principal().tenant_id(),
                    workspace_id,
                    project_id: None,
                },
                SecurityClassification::Public,
                None,
                ExecutionBudget::default(),
            )
            .await?;
        Ok((
            workspace_id.as_uuid(),
            scope.classification_ceiling.min(app.max_classification()),
        ))
    }

    pub async fn list_conversations(&self) -> ServiceResult<Vec<ConversationSummary>> {
        let (workspace, _) = self
            .conversation_workspace(PolicyAction::ReadEvidence)
            .await?;
        let rows = sqlx::query(
            "SELECT id,source,source_conversation_id,title,capture_mode,created_at \
             FROM conversation_threads WHERE tenant_id=$1 AND workspace_id=$2 AND owner_principal_id=$3 \
             ORDER BY created_at DESC LIMIT 100")
            .bind(self.principal().tenant_id().as_uuid()).bind(workspace)
            .bind(self.principal().id().as_uuid()).fetch_all(self.app().pool()).await?;
        rows.into_iter()
            .map(|row| {
                Ok(ConversationSummary {
                    id: row.try_get("id")?,
                    source: row.try_get("source")?,
                    source_conversation_id: row.try_get("source_conversation_id")?,
                    title: row.try_get("title")?,
                    capture_mode: row.try_get("capture_mode")?,
                    created_at: row.try_get("created_at")?,
                })
            })
            .collect()
    }

    pub async fn conversation_events(
        &self,
        thread_id: Uuid,
    ) -> ServiceResult<Vec<ConversationEvent>> {
        let (workspace, ceiling) = self
            .conversation_workspace(PolicyAction::ReadEvidence)
            .await?;
        let allowed = SecurityClassification::allowed_up_to(ceiling);
        let rows = sqlx::query(
            "SELECT e.id,e.sequence,e.event_type,e.actor_type,e.content,e.occurred_at \
             FROM conversation_events e JOIN conversation_threads t ON t.id=e.thread_id AND t.tenant_id=e.tenant_id \
             WHERE t.id=$1 AND t.tenant_id=$2 AND t.workspace_id=$3 AND t.owner_principal_id=$4 \
             AND e.security_classification=ANY($5) \
             ORDER BY e.sequence LIMIT 1000")
            .bind(thread_id).bind(self.principal().tenant_id().as_uuid()).bind(workspace)
            .bind(self.principal().id().as_uuid()).bind(allowed).fetch_all(self.app().pool()).await?;
        rows.into_iter()
            .map(|row| {
                Ok(ConversationEvent {
                    id: row.try_get("id")?,
                    sequence: row.try_get("sequence")?,
                    event_type: row.try_get("event_type")?,
                    actor_type: row.try_get("actor_type")?,
                    content: row.try_get("content")?,
                    occurred_at: row.try_get("occurred_at")?,
                })
            })
            .collect()
    }

    pub async fn capture_conversation(
        &self,
        input: CaptureConversation,
    ) -> ServiceResult<CapturedEvent> {
        if input.source.is_empty()
            || input.source.len() > 100
            || input.source_conversation_id.is_empty()
            || input.source_conversation_id.len() > 500
            || input.title.as_ref().is_some_and(|s| s.len() > 500)
            || input
                .source_event_id
                .as_ref()
                .is_some_and(|s| s.is_empty() || s.len() > 500)
            || input.content.as_ref().is_some_and(|s| s.len() > 256 * 1024)
        {
            return Err(ServiceError::validation(
                "conversation field exceeds its size limit",
            ));
        }
        if !valid_enum(
            &input.capture_mode,
            &[
                "MCP_ONLY",
                "PARTIAL_ADAPTER",
                "NATIVE_COMPLETE",
                "IMPORTED",
                "UNKNOWN",
            ],
        ) || !valid_enum(
            &input.event_type,
            &[
                "USER_MESSAGE",
                "ASSISTANT_MESSAGE",
                "TOOL_CALL",
                "TOOL_RESULT",
                "SYSTEM_EVENT",
                "ARTIFACT_CREATED",
            ],
        ) || !valid_enum(&input.actor_type, &["USER", "ASSISTANT", "TOOL", "SYSTEM"])
        {
            return Err(ServiceError::validation(
                "invalid conversation event type or capture mode",
            ));
        }
        // A model-mediated call cannot certify that it saw the entire host transcript.
        if input.capture_mode == "NATIVE_COMPLETE" {
            return Err(ServiceError::validation(
                "complete capture requires a verified host adapter",
            ));
        }
        let classification = match input
            .security_classification
            .as_deref()
            .unwrap_or("INTERNAL")
        {
            "PUBLIC" => SecurityClassification::Public,
            "INTERNAL" => SecurityClassification::Internal,
            "CONFIDENTIAL" => SecurityClassification::Confidential,
            "RESTRICTED" => SecurityClassification::Restricted,
            "SECRET" => SecurityClassification::Secret,
            _ => return Err(ServiceError::validation("invalid security classification")),
        };
        let app = self.app();
        let (workspace_id, _) =
            policy::project_workspace_for_principal(app.pool(), self.principal())
                .await?
                .ok_or_else(|| ServiceError::forbidden("principal has no active workspace"))?;
        self.authorize(
            PolicyAction::AppendEvidence,
            ResourceScope {
                tenant_id: self.principal().tenant_id(),
                workspace_id,
                project_id: None,
            },
            classification,
            None,
            ExecutionBudget::default(),
        )
        .await?;
        let tenant = self.principal().tenant_id().as_uuid();
        let owner = self.principal().id().as_uuid();
        let workspace = workspace_id.as_uuid();
        let mut tx = app.pool().begin().await?;
        let thread_id = Uuid::new_v4();
        let row = sqlx::query(
            "INSERT INTO conversation_threads(id,tenant_id,workspace_id,owner_principal_id,source,source_conversation_id,title,capture_mode) \
             VALUES($1,$2,$3,$4,$5,$6,$7,$8) \
             ON CONFLICT(tenant_id,workspace_id,owner_principal_id,source,source_conversation_id) \
             DO UPDATE SET title=COALESCE(conversation_threads.title,EXCLUDED.title) \
             RETURNING id")
            .bind(thread_id).bind(tenant).bind(workspace).bind(owner)
            .bind(&input.source).bind(&input.source_conversation_id).bind(&input.title)
            .bind(&input.capture_mode).fetch_one(&mut *tx).await?;
        let thread_id: Uuid = row.try_get("id")?;
        // Lock the thread so concurrent appends allocate a stable sequence.
        sqlx::query("SELECT id FROM conversation_threads WHERE id=$1 FOR UPDATE")
            .bind(thread_id)
            .fetch_one(&mut *tx)
            .await?;
        if let Some(source_event_id) = &input.source_event_id {
            let existing = sqlx::query("SELECT id,sequence FROM conversation_events WHERE thread_id=$1 AND source_event_id=$2")
                .bind(thread_id).bind(source_event_id).fetch_optional(&mut *tx).await?;
            if let Some(row) = existing {
                return Ok(CapturedEvent {
                    thread_id,
                    event_id: row.try_get("id")?,
                    sequence: row.try_get("sequence")?,
                    duplicate: true,
                });
            }
        }
        let next: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(sequence),-1)+1 FROM conversation_events WHERE thread_id=$1",
        )
        .bind(thread_id)
        .fetch_one(&mut *tx)
        .await?;
        let event_id = Uuid::new_v4();
        sqlx::query("INSERT INTO conversation_events(id,tenant_id,thread_id,source_event_id,sequence,event_type,actor_type,content,security_classification,occurred_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
            .bind(event_id).bind(tenant).bind(thread_id).bind(&input.source_event_id).bind(next)
            .bind(&input.event_type).bind(&input.actor_type).bind(&input.content)
            .bind(input.security_classification.as_deref().unwrap_or("INTERNAL"))
            .bind(input.occurred_at.unwrap_or_else(Utc::now)).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(CapturedEvent {
            thread_id,
            event_id,
            sequence: next,
            duplicate: false,
        })
    }
}
