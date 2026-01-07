use std::net::SocketAddr;
use std::sync::Arc;

use crate::{repository::{DashboardRepository, RuleRepository}, router::create_router};

/// Run the API server
pub async fn run_server(
    dashboard_repo: Arc<dyn DashboardRepository>,
    rule_repo: Arc<dyn RuleRepository>,
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

    let app = create_router(dashboard_repo, rule_repo);

    let addr = format!("{}:{}", host, port).parse::<SocketAddr>()?;
    tracing::info!("Starting server on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
