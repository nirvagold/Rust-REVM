//! API Route Configuration

use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

use super::handlers::{self, AppState};
use super::middleware::{auth_middleware, logging_middleware, rate_limit_middleware};

/// Create the API router with all routes and middleware
pub fn create_router(state: Arc<AppState>) -> Router {
    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);
    
    // API v1 routes
    let api_v1 = Router::new()
        // Health & Status
        .route("/health", get(handlers::health_check))
        .route("/stats", get(handlers::get_stats))
        // Token Analysis
        .route("/analyze/token", post(handlers::analyze_token))
        .route("/honeypot/check", post(handlers::check_honeypot))
        // Batch Analysis (NEW!)
        .route("/analyze/batch", post(handlers::batch_analyze));
    
    // Build full router
    Router::new()
        .nest("/v1", api_v1)
        // Also expose at root for convenience
        .route("/health", get(handlers::health_check))
        .with_state(state)
        // Middleware (order matters - bottom runs first)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .layer(middleware::from_fn(logging_middleware))
        .layer(middleware::from_fn(rate_limit_middleware))
        .layer(middleware::from_fn(auth_middleware))
}
