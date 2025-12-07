use std::net::SocketAddr;
use std::sync::Arc;

use crate::{repository::DashboardRepository, router::create_router};

/// Run the API server
pub async fn run_server(
    repo: Arc<dyn DashboardRepository>,
    host: &str,
    port: u16,
) -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "backend_api=debug,tower_http=debug,axum=trace".into()),
        )
        .init();

    let app = create_router(repo);

    let addr = format!("{}:{}", host, port).parse::<SocketAddr>()?;
    tracing::info!("Starting server on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
