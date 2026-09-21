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
            AppState::business(services)
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
