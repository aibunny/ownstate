//! Background job processing. The worker binary and tests both call
//! [`AppServices::process_next_job`]; there is exactly one implementation of
//! each job kind.

use chrono::{Duration, Utc};
use ownstate_domain::{
    ExecutionBudget, Job, JobId, JobKind, KnowledgeVersionId, ModelDestination, PolicyAction,
    PolicyRequest,
};
use ownstate_storage::{embeddings, jobs, knowledge, policy};
use pgvector::Vector;
use serde::Deserialize;

use crate::policy::PrincipalServices;
use crate::{AppServices, ServiceError, ServiceResult};

#[derive(Debug, PartialEq, Eq)]
pub enum JobOutcome {
    /// A job was claimed and finished (successfully or parked for retry).
    Processed(JobId),
    QueueEmpty,
}

#[derive(Deserialize)]
struct GenerateEmbeddingPayload {
    knowledge_version_id: KnowledgeVersionId,
}

/// How long a claimed job may sit RUNNING before it is presumed orphaned by
/// a crashed/stopped worker and requeued.
const STALE_JOB_AFTER_SECS: i64 = 300;

impl AppServices {
    #[allow(dead_code)]
    pub(crate) async fn process_next_job(&self) -> ServiceResult<JobOutcome> {
        let requeued =
            jobs::requeue_stale(self.pool(), Duration::seconds(STALE_JOB_AFTER_SECS)).await?;
        if requeued > 0 {
            tracing::warn!(requeued, "requeued jobs orphaned by a stopped worker");
        }

        let Some(job) = jobs::claim_next(self.pool()).await? else {
            return Ok(JobOutcome::QueueEmpty);
        };

        let job_id = job.id;
        match self.run_job(&job).await {
            Ok(()) => {
                jobs::complete(self.pool(), job_id).await?;
                tracing::info!(job_id = %job_id, kind = %job.kind, "job succeeded");
            }
            Err(err) => {
                // Backoff grows with attempts; storage parks the job as
                // FAILED once max_attempts is exhausted.
                let retry_at = Utc::now() + Duration::seconds(30 * i64::from(job.attempts));
                tracing::warn!(job_id = %job_id, kind = %job.kind, error = %err, "job failed");
                jobs::fail(self.pool(), job_id, &err.to_string(), retry_at).await?;
            }
        }
        Ok(JobOutcome::Processed(job_id))
    }

    async fn run_job(&self, job: &Job) -> ServiceResult<()> {
        match job.kind {
            JobKind::GenerateEmbedding => {
                let payload: GenerateEmbeddingPayload = serde_json::from_value(job.payload.clone())
                    .map_err(|e| ServiceError::validation(format!("invalid job payload: {e}")))?;
                self.generate_embedding(payload.knowledge_version_id).await
            }
        }
    }

    async fn generate_embedding(&self, version_id: KnowledgeVersionId) -> ServiceResult<()> {
        let version = knowledge::get_version_unscoped(self.pool(), version_id).await?;

        let mut vectors = self
            .embedder()
            .embed(std::slice::from_ref(&version.content))
            .await
            .map_err(|e| ServiceError::Embedding(e.to_string()))?;
        if vectors.is_empty() {
            return Err(ServiceError::Embedding(
                "provider returned no vectors".into(),
            ));
        }
        let vector = vectors.remove(0);
        if vector.len() != self.embedder().dimensions() {
            return Err(ServiceError::Embedding(format!(
                "provider returned {} dims, expected {}",
                vector.len(),
                self.embedder().dimensions()
            )));
        }

        embeddings::upsert_knowledge_version_embedding(
            self.pool(),
            embeddings::NewEmbedding {
                entity_id: version.id,
                model: self.embedder().model_name(),
                model_version: self.embedder().model_version(),
                dimensions: vector.len() as i32,
                vector: Vector::from(vector),
            },
        )
        .await?;
        Ok(())
    }

    /// Drain the queue (tests and CLI convenience). Returns processed count.
    #[allow(dead_code)]
    pub(crate) async fn drain_jobs(&self, max: usize) -> ServiceResult<usize> {
        let mut processed = 0;
        while processed < max {
            match self.process_next_job().await? {
                JobOutcome::Processed(_) => processed += 1,
                JobOutcome::QueueEmpty => break,
            }
        }
        Ok(processed)
    }
}

impl PrincipalServices {
    /// Claim and process a job as the dedicated worker principal. Policy is
    /// checked before content load and again, under shared grant locks, before
    /// the derived embedding is written.
    pub async fn process_next_job(&self) -> ServiceResult<JobOutcome> {
        let requeued =
            jobs::requeue_stale(self.app().pool(), Duration::seconds(STALE_JOB_AFTER_SECS)).await?;
        if requeued > 0 {
            tracing::warn!(requeued, "requeued jobs orphaned by a stopped worker");
        }
        let Some(job) = jobs::claim_next(self.app().pool()).await? else {
            return Ok(JobOutcome::QueueEmpty);
        };
        let job_id = job.id;
        let result = match job.kind {
            JobKind::GenerateEmbedding => {
                let payload: GenerateEmbeddingPayload = serde_json::from_value(job.payload.clone())
                    .map_err(|error| {
                        ServiceError::validation(format!("invalid job payload: {error}"))
                    })?;
                self.generate_embedding(payload.knowledge_version_id).await
            }
        };
        match result {
            Ok(()) => {
                jobs::complete(self.app().pool(), job_id).await?;
                tracing::info!(job_id = %job_id, kind = %job.kind, "job succeeded");
            }
            Err(error) => {
                let retry_at = Utc::now() + Duration::seconds(30 * i64::from(job.attempts));
                tracing::warn!(job_id = %job_id, kind = %job.kind, error = %error, "job failed");
                jobs::fail(self.app().pool(), job_id, &error.to_string(), retry_at).await?;
            }
        }
        Ok(JobOutcome::Processed(job_id))
    }

    async fn generate_embedding(&self, version_id: KnowledgeVersionId) -> ServiceResult<()> {
        let resource = policy::resource_for_knowledge_version(
            self.app().pool(),
            self.app().tenant_id(),
            version_id,
        )
        .await?;
        self.authorize_resource(
            PolicyAction::ReadKnowledge,
            resource.clone(),
            self.app().max_classification(),
        )
        .await?;
        self.authorize_local_embedding(
            resource.clone(),
            self.app().max_classification(),
            ExecutionBudget::default(),
        )
        .await?;
        let version = knowledge::get_version_unscoped(self.app().pool(), version_id).await?;
        let mut vectors = self
            .app()
            .embedder()
            .embed(std::slice::from_ref(&version.content))
            .await
            .map_err(|error| ServiceError::Embedding(error.to_string()))?;
        if vectors.is_empty() {
            return Err(ServiceError::Embedding(
                "provider returned no vectors".into(),
            ));
        }
        let vector = vectors.remove(0);
        if vector.len() != self.app().embedder().dimensions() {
            return Err(ServiceError::Embedding(format!(
                "provider returned {} dims, expected {}",
                vector.len(),
                self.app().embedder().dimensions()
            )));
        }
        let provider = match self.app().embedder().model_name() {
            "deterministic-hash" => "deterministic",
            _ => "fastembed",
        };
        let mut tx = self.app().pool().begin().await?;
        policy::set_local_policy_context(&mut tx, self.principal()).await?;
        let current_resource =
            policy::resource_for_knowledge_version(&mut *tx, self.app().tenant_id(), version_id)
                .await?;
        for (action, destination) in [
            (PolicyAction::ReadKnowledge, None),
            (PolicyAction::GenerateEmbedding, None),
            (
                PolicyAction::ModelEgress,
                Some(ModelDestination::Local(provider.to_string())),
            ),
        ] {
            let request = PolicyRequest {
                action,
                resource: current_resource.clone(),
                classification: self.app().max_classification(),
                destination,
                budget: ExecutionBudget::default(),
                now: Utc::now(),
                execution_id: None,
            };
            if policy::authorize_and_record(&mut tx, self.principal(), &request)
                .await?
                .is_none()
            {
                tx.commit().await?;
                return Err(ServiceError::forbidden(
                    "worker authorization was revoked before embedding write",
                ));
            }
        }
        embeddings::upsert_knowledge_version_embedding(
            &mut *tx,
            embeddings::NewEmbedding {
                entity_id: version.id,
                model: self.app().embedder().model_name(),
                model_version: self.app().embedder().model_version(),
                dimensions: vector.len() as i32,
                vector: Vector::from(vector),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn drain_jobs(&self, max: usize) -> ServiceResult<usize> {
        let mut processed = 0;
        while processed < max {
            match self.process_next_job().await? {
                JobOutcome::Processed(_) => processed += 1,
                JobOutcome::QueueEmpty => break,
            }
        }
        Ok(processed)
    }
}
