//! Shared proposal, curation, entity resolution, and temporal reads.
use chrono::{DateTime, Utc};
use ownstate_domain::*;
use ownstate_storage::{events, institutional, projects, query};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AppServices, ServiceError, ServiceResult};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposeInstitutional {
    pub project_id: ProjectId,
    pub proposal: GraphProposal,
    pub evidence_event_ids: Vec<EventId>,
    pub security_classification: Option<SecurityClassification>,
    pub observed_at: Option<DateTime<Utc>>,
    pub proposed_by: ProposerKind,
    pub confidence: Option<f32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstitutionalQuery {
    pub project_id: ProjectId,
    pub as_of: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct InstitutionalSnapshot {
    pub as_of: DateTime<Utc>,
    pub versions: Vec<InstitutionalVersion>,
}

impl AppServices {
    async fn institutional_endpoints(
        &self,
        proposal: &GraphProposal,
        project: ProjectId,
        allowed: &[&str],
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> ServiceResult<()> {
        let endpoints = match proposal {
            GraphProposal::Entity { .. } => Vec::new(),
            GraphProposal::Claim {
                subject_entity_id, ..
            } => vec![*subject_entity_id],
            GraphProposal::Relationship {
                source_entity_id,
                target_entity_id,
                ..
            } => vec![*source_entity_id, *target_entity_id],
        };
        let as_of = query::snapshot_timestamp(&mut **tx).await?;
        for id in endpoints {
            let visible = institutional::versions(
                &mut **tx,
                institutional::InstitutionalRead {
                    tenant: self.tenant_id(),
                    project,
                    record_id: Some(id.as_uuid()),
                    allowed,
                    as_of,
                    record_type: Some("ENTITY"),
                    endpoint: None,
                    limit: 1,
                },
            )
            .await?;
            if visible.is_empty() {
                return Err(ServiceError::validation(
                    "assertion endpoint is not an accessible current entity in this project",
                ));
            }
        }
        Ok(())
    }

    pub(crate) async fn propose_institutional(
        &self,
        req: ProposeInstitutional,
    ) -> ServiceResult<GraphCandidate> {
        req.proposal.validate().map_err(ServiceError::validation)?;
        if req.confidence.is_some_and(|c| !(0.0..=1.0).contains(&c)) {
            return Err(ServiceError::validation("confidence must be within [0,1]"));
        }
        if req.evidence_event_ids.is_empty()
            || req.evidence_event_ids.len() > limits::MAX_EVIDENCE_REFS
        {
            return Err(ServiceError::validation(
                "institutional proposals require bounded nonempty evidence",
            ));
        }
        let mut tx = self.pool().begin().await?;
        projects::get(&mut *tx, self.tenant_id(), req.project_id).await?;
        let evidence = events::get_scoped_by_ids(
            &mut *tx,
            self.tenant_id(),
            req.project_id,
            &req.evidence_event_ids,
        )
        .await?;
        if evidence.len() != req.evidence_event_ids.len() {
            return Err(ServiceError::validation(
                "evidence is duplicated or outside project scope",
            ));
        }
        let classification = SecurityClassification::with_evidence_floor(
            req.security_classification,
            evidence.iter().map(|e| e.security_classification),
        );
        if classification > self.max_classification() {
            return Err(ServiceError::validation(
                "proposal exceeds authorized classification",
            ));
        }
        let allowed = SecurityClassification::allowed_up_to(classification);
        self.institutional_endpoints(&req.proposal, req.project_id, &allowed, &mut tx)
            .await?;
        let c = GraphCandidate {
            id: GraphCandidateId::generate(),
            tenant_id: self.tenant_id(),
            project_id: req.project_id,
            proposal: req.proposal,
            evidence_event_ids: req.evidence_event_ids,
            security_classification: classification,
            proposed_by: req.proposed_by,
            confidence: req.confidence,
            status: CandidateStatus::Pending,
            observed_at: req.observed_at.unwrap_or_else(Utc::now),
            created_at: Utc::now(),
        };
        institutional::insert_candidate(&mut *tx, &c).await?;
        tx.commit().await?;
        Ok(c)
    }

    pub(crate) async fn promote_institutional(
        &self,
        id: GraphCandidateId,
    ) -> ServiceResult<InstitutionalVersion> {
        let mut tx = self.pool().begin().await?;
        let c = institutional::lock_candidate(&mut tx, self.tenant_id(), id).await?;
        let evidence = events::get_scoped_by_ids(
            &mut *tx,
            self.tenant_id(),
            c.project_id,
            &c.evidence_event_ids,
        )
        .await?;
        if evidence.len() != c.evidence_event_ids.len() || evidence.is_empty() {
            return Err(ServiceError::validation(
                "canonical institutional state requires scoped evidence",
            ));
        }
        let classification = SecurityClassification::with_evidence_floor(
            Some(c.security_classification),
            evidence.iter().map(|e| e.security_classification),
        );
        if classification > self.max_classification() {
            return Err(ServiceError::validation(
                "candidate exceeds authorized classification",
            ));
        }
        self.institutional_endpoints(
            &c.proposal,
            c.project_id,
            &SecurityClassification::allowed_up_to(classification),
            &mut tx,
        )
        .await?;
        let trust = if c.proposed_by == ProposerKind::Human {
            TrustLevel::HumanExplicit
        } else {
            TrustLevel::AgentDerived
        };
        let v = institutional::promote_candidate(&mut tx, &c, trust, classification).await?;
        tx.commit().await?;
        tracing::info!(candidate_id=%id,version_id=%v.id,status=%v.status,"institutional candidate curated");
        Ok(v)
    }

    pub(crate) async fn institutional_snapshot(
        &self,
        q: InstitutionalQuery,
        record: Option<Uuid>,
        kind: Option<&str>,
    ) -> ServiceResult<InstitutionalSnapshot> {
        projects::get(self.pool(), self.tenant_id(), q.project_id).await?;
        let limit = q.limit.unwrap_or(100);
        if limit == 0 || limit > 1000 {
            return Err(ServiceError::validation("limit must be 1..=1000"));
        }
        let as_of = match q.as_of {
            Some(at) => at,
            None => query::snapshot_timestamp(self.pool()).await?,
        };
        let versions = institutional::versions(
            self.pool(),
            institutional::InstitutionalRead {
                tenant: self.tenant_id(),
                project: q.project_id,
                record_id: record,
                allowed: &SecurityClassification::allowed_up_to(self.max_classification()),
                as_of,
                record_type: kind,
                endpoint: None,
                limit: limit as i64,
            },
        )
        .await?;
        Ok(InstitutionalSnapshot { as_of, versions })
    }

    pub(crate) async fn entity_relationships(
        &self,
        project: ProjectId,
        id: EntityId,
        limit: usize,
        as_of: Option<DateTime<Utc>>,
    ) -> ServiceResult<Vec<InstitutionalVersion>> {
        projects::get(self.pool(), self.tenant_id(), project).await?;
        if limit == 0 || limit > 1000 {
            return Err(ServiceError::validation("limit must be 1..=1000"));
        }
        Ok(institutional::versions(
            self.pool(),
            institutional::InstitutionalRead {
                tenant: self.tenant_id(),
                project,
                record_id: None,
                allowed: &SecurityClassification::allowed_up_to(self.max_classification()),
                as_of: match as_of {
                    Some(at) => at,
                    None => query::snapshot_timestamp(self.pool()).await?,
                },
                record_type: Some("RELATIONSHIP"),
                endpoint: Some(id),
                limit: limit as i64,
            },
        )
        .await?)
    }

    pub(crate) async fn resolve_entity(
        &self,
        project: ProjectId,
        value: &str,
        namespace: Option<&str>,
        as_of: Option<DateTime<Utc>>,
    ) -> ServiceResult<EntityResolution> {
        projects::get(self.pool(), self.tenant_id(), project).await?;
        if value.trim().is_empty()
            || value.len() > 200
            || namespace.is_some_and(|n| n.is_empty() || n.len() > 200)
        {
            return Err(ServiceError::validation("invalid resolution identifier"));
        }
        Ok(institutional::resolve(
            self.pool(),
            self.tenant_id(),
            project,
            value,
            namespace,
            &SecurityClassification::allowed_up_to(self.max_classification()),
            match as_of {
                Some(at) => at,
                None => query::snapshot_timestamp(self.pool()).await?,
            },
        )
        .await?)
    }

    pub(crate) async fn institutional_history(
        &self,
        project: ProjectId,
        record: Uuid,
    ) -> ServiceResult<Vec<InstitutionalVersion>> {
        projects::get(self.pool(), self.tenant_id(), project).await?;
        Ok(institutional::history(
            self.pool(),
            self.tenant_id(),
            project,
            record,
            &SecurityClassification::allowed_up_to(self.max_classification()),
        )
        .await?)
    }
}
