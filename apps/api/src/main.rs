use ownstate_api::{AppState, build_router};
use ownstate_runtime::{Config, DeploymentMode, RuntimeError, build_services, init_tracing};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    init_tracing("info,ownstate_api=debug");

    if let Err(err) = run().await {
        tracing::error!(error = %err, "api startup failed");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    let services = build_services(&config).await?;
    let mut mcp_allowed_hosts = vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
    ];
    if let Ok(host) = std::env::var("RAILWAY_PUBLIC_DOMAIN")
        && !host.trim().is_empty()
    {
        mcp_allowed_hosts.push(host);
    }
    if let Ok(hosts) = std::env::var("OWNSTATE_MCP_ALLOWED_HOSTS") {
        mcp_allowed_hosts.extend(
            hosts
                .split(',')
                .map(str::trim)
                .filter(|host| !host.is_empty())
                .map(str::to_string),
        );
    }

    if config.deployment_mode == DeploymentMode::Personal
        && config.api_token.is_none()
        && config.admin_token.is_none()
    {
        tracing::warn!("OWNSTATE_API_TOKEN is not set: API runs UNAUTHENTICATED (dev only)");
    }

    let state = match config.deployment_mode {
        DeploymentMode::Personal => {
            AppState::new(
                services,
                config.api_token.clone(),
                config.admin_token.clone(),
                config.mcp_token.clone(),
                mcp_allowed_hosts,
            )
            .await?
        }
        DeploymentMode::Business => {
            if config.api_token.is_none() && config.admin_token.is_none() {
                return Err(RuntimeError::Config(
                    "Business API requires an explicitly provisioned bearer credential".into(),
                )
                .into());
            }
            for token in [config.api_token.as_ref(), config.admin_token.as_ref()]
                .into_iter()
                .flatten()
            {
                services
                    .authenticate_digest(&ownstate_domain::content_hash(token.as_bytes()))
                    .await?;
            }
            let mcp_principal = match config.mcp_token.as_ref() {
                Some(token) => Some(
                    services
                        .authenticate_digest(&ownstate_domain::content_hash(token.as_bytes()))
                        .await?,
                ),
                None => None,
            };
            AppState::business(services, mcp_principal, mcp_allowed_hosts)
        }
    };
    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind(config.http_addr).await?;
    tracing::info!(addr = %config.http_addr, "ownstate api listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    tracing::info!("api shut down cleanly");
    Ok(())
}

async fn shutdown_signal() {
    // Both SIGINT and SIGTERM trigger graceful shutdown.
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received");
}
