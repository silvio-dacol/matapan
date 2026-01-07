use axum::{
    routing::{delete, get, post, put},
    Router,
};
use serde_json::json;
use std::sync::Arc;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

use crate::{rule_handlers, repository::RuleRepository};

/// Health check endpoint
async fn health_check() -> axum::Json<serde_json::Value> {
    axum::Json(json!({
        "status": "healthy",
        "service": "matapan-rule-engine-api"
    }))
}

/// Create the main application router with rule engine endpoints
pub fn create_router(rule_repo: Arc<dyn RuleRepository>) -> Router {
    // Create CORS layer
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build the router
    Router::new()
        // Health check
        .route("/health", get(health_check))
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
        // Add shared state
        .with_state(rule_repo)
        // Add middleware
        .layer(cors)
        .layer(TraceLayer::new_for_http())
}
