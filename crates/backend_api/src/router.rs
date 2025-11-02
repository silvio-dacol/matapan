use axum::{routing::get, Router};
use std::sync::Arc;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

use crate::{handlers, repository::DashboardRepository};

/// Create the main application router with all API endpoints
pub fn create_router(repo: Arc<dyn DashboardRepository>) -> Router {
    // Create CORS layer
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build the router
    Router::new()
        // Health check
        .route("/health", get(handlers::health_check))
        // Dashboard endpoints
        .route("/api/dashboard", get(handlers::get_dashboard))
        .route("/api/dashboard/latest", get(handlers::get_latest_snapshot))
        .route("/api/dashboard/summary", get(handlers::get_summary))
        // Snapshot entries endpoints
        .route(
            "/api/snapshots/:date/entries",
            get(handlers::get_snapshot_entries),
        )
        .route(
            "/api/snapshots/:date/entries/enriched",
            get(handlers::get_snapshot_entries_enriched),
        )
        // Add shared state
        .with_state(repo)
        // Add middleware
        .layer(cors)
        .layer(TraceLayer::new_for_http())
}
