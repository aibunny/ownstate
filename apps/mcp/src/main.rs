//! Ownstate MCP server over stdio. Logs go to stderr; stdout carries the
//! JSON-RPC stream.

use ownstate_mcp::OwnstateMcp;
use ownstate_runtime::{Config, DeploymentMode, RuntimeError, build_services, init_tracing};
use rmcp::ServiceExt;
use rmcp::transport::stdio;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    init_tracing("info,ownstate_mcp=debug");

    if let Err(err) = run().await {
        tracing::error!(error = %err, "mcp server failed");
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
                .mcp
        }
        DeploymentMode::Business => {
            let token = config.mcp_token.as_ref().ok_or_else(|| {
                RuntimeError::Config(
                    "Business MCP requires OWNSTATE_MCP_TOKEN for a provisioned principal".into(),
                )
            })?;
            services
                .authenticate_digest(&ownstate_domain::content_hash(token.as_bytes()))
                .await?
        }
    };
    let scoped = services.for_principal(principal)?;

    let workspace = ownstate_mcp::workspace::detect();
    match &workspace {
        Some(w) => tracing::info!(
            workspace = %w.name,
            git_origin = w.git_origin.as_deref().unwrap_or("-"),
            "workspace detected; tools default to this project"
        ),
        None => tracing::warn!("no workspace detected; tools require an explicit project"),
    }

    let server = OwnstateMcp::new(scoped, workspace).serve(stdio()).await?;
    tracing::info!("ownstate mcp server ready on stdio");
    server.waiting().await?;
    Ok(())
}
