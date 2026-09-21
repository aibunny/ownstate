//! Background worker: claims durable jobs from PostgreSQL and processes them
//! through the same application services as every other binary.

use std::time::Duration;

use ownstate_runtime::{Config, DeploymentMode, RuntimeError, build_services, init_tracing};
use ownstate_services::jobs::JobOutcome;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    init_tracing("info,ownstate_worker=debug");

    if let Err(err) = run().await {
        tracing::error!(error = %err, "worker startup failed");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    let services = build_services(&config).await?;
    let principal = match config.deployment_mode {
        DeploymentMode::Personal => {
            services
                .bootstrap_personal_policy_actors(None, None)
                .await?
                .worker
        }
        DeploymentMode::Business => {
            let token = config.worker_token.as_ref().ok_or_else(|| {
                RuntimeError::Config(
                    "Business worker requires OWNSTATE_WORKER_TOKEN for a provisioned principal"
                        .into(),
                )
            })?;
            services
                .authenticate_digest(&ownstate_domain::content_hash(token.as_bytes()))
                .await?
        }
    };
    let services = services.for_principal(principal)?;
    let poll = Duration::from_millis(config.worker_poll_ms);
    tracing::info!(poll_ms = config.worker_poll_ms, "ownstate worker started");

    loop {
        tokio::select! {
            biased;
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("shutdown signal received; worker exiting");
                return Ok(());
            }
            outcome = services.process_next_job() => {
                match outcome {
                    Ok(JobOutcome::Processed(_)) => {
                        // Immediately look for the next job.
                    }
                    Ok(JobOutcome::QueueEmpty) => {
                        tokio::time::sleep(poll).await;
                    }
                    Err(err) => {
                        // Claim/complete failures are infrastructure issues;
                        // back off instead of spinning.
                        tracing::error!(error = %err, "job processing error");
                        tokio::time::sleep(poll).await;
                    }
                }
            }
        }
    }
}
