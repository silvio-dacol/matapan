use axum::{
    routing::{delete, get, post, put},
    Router,
};
use std::sync::Arc;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

use crate::{handlers, rule_handlers, repository::{DashboardRepository, RuleRepository}};

/// Create the main application router with all API endpoints
pub fn create_router(
    dashboard_repo: Arc<dyn DashboardRepository>,
    rule_repo: Arc<dyn RuleRepository>,
) -> Router {
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
        // Snapshot entries endpoint
        .route(
            "/api/snapshots/:date/entries",
            get(handlers::get_snapshot_entries),
        )
        // Cache management
        .route("/api/cache/invalidate", post(handlers::invalidate_cache))
        // Add shared state for dashboard
        .with_state(dashboard_repo)
        // Rule management endpoints
        .route("/api/rules", get(rule_handlers::get_rules))
        .route("/api/rules", post(rule_handlers::create_rule))
        .route("/api/rules/:rule_id", get(rule_handlers::get_rule))
        .route("/api/rules/:rule_id", put(rule_handlers::update_rule))
        .route("/api/rules/:rule_id", delete(rule_handlers::delete_rule))
        .route("/api/rules/:rule_id/toggle", post(rule_handlers::toggle_rule))
        .route("/api/rules/test", post(rule_handlers::test_rule))
        .route("/api/rules/apply", post(rule_handlers::apply_rules))
        .route("/api/rules/reorder", post(rule_handlers::reorder_rules))
        // Add shared state for rules
        .with_state(rule_repo)
        // Add middleware
        .layer(cors)
        .layer(TraceLayer::new_for_http())
}
