//! Ruster REVM Cloud API Server
//!
//! High-performance REST API for token risk analysis using PERS algorithm
//!
//! Usage:
//!   cargo run --bin ruster_api
//!   
//! Environment:
//!   RUSTER_PORT - Server port (default: 3000)
//!   RUSTER_HOST - Server host (default: 0.0.0.0)
//!   RUST_LOG    - Log level (default: info)

use ruster_revm::api::{create_router, handlers::AppState, start_cleanup_task};
use ruster_revm::TelemetryCollector;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Initialize logging
    FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .compact()
        .init();

    print_banner();

    // Initialize telemetry
    let telemetry = Arc::new(TelemetryCollector::new());
    let telemetry_for_shutdown = telemetry.clone();

    // Create app state
    let state = Arc::new(AppState::new(telemetry));

    // Start background cleanup task for rate limiter
    start_cleanup_task();
    info!("🧹 Background cleanup task started");

    // Create router
    let app = create_router(state);

    // Get server config from env
    // Railway uses PORT env var, fallback to RUSTER_PORT for local dev
    let host = std::env::var("RUSTER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port: u16 = std::env::var("PORT")
        .or_else(|_| std::env::var("RUSTER_PORT"))
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;

    info!("🚀 Ruster REVM API starting on http://{}", addr);
    info!("📚 API Documentation: http://{}/v1/health", addr);
    info!("");
    info!("Endpoints:");
    info!("  POST /v1/analyze/token    - Full token risk analysis (PERS)");
    info!("  POST /v1/honeypot/check   - Quick honeypot detection");
    info!("  POST /v1/analyze/batch    - Batch analysis (up to 100 tokens)");
    info!("  GET  /v1/stats            - Protection statistics");
    info!("  GET  /v1/health           - Health check");
    info!("");
    info!("Press Ctrl+C for graceful shutdown");
    info!("");

    // Start server with graceful shutdown
    let listener = TcpListener::bind(addr).await?;

    // Create shutdown signal handler
    let shutdown_signal = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    // Run server with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await?;

    // Graceful shutdown sequence
    info!("");
    info!("🛑 Shutdown signal received, cleaning up...");

    // Export final telemetry
    info!("📊 Exporting final telemetry...");
    let stats = telemetry_for_shutdown.get_stats();
    info!("   Total analyzed: {}", stats.total_analyzed);
    info!("   Total threats: {}", stats.total_threats);
    info!("   Honeypots detected: {}", stats.honeypots_detected);

    // Try to export stats to file
    match telemetry_for_shutdown.export_stats_json() {
        Ok(path) => info!("   ✅ Stats exported to: {}", path.display()),
        Err(e) => warn!("   ⚠️ Failed to export stats: {}", e),
    }

    info!("👋 Ruster REVM API shutdown complete");

    Ok(())
}

fn print_banner() {
    println!(
        r#"
    ╔══════════════════════════════════════════════════════════════╗
    ║                                                              ║
    ║   ██████╗ ██╗   ██╗███████╗████████╗███████╗██████╗          ║
    ║   ██╔══██╗██║   ██║██╔════╝╚══██╔══╝██╔════╝██╔══██╗         ║
    ║   ██████╔╝██║   ██║███████╗   ██║   █████╗  ██████╔╝         ║
    ║   ██╔══██╗██║   ██║╚════██║   ██║   ██╔══╝  ██╔══██╗         ║
    ║   ██║  ██║╚██████╔╝███████║   ██║   ███████╗██║  ██║         ║
    ║   ╚═╝  ╚═╝ ╚═════╝ ╚══════╝   ╚═╝   ╚══════╝╚═╝  ╚═╝         ║
    ║                      R E V M                                 ║
    ║                                                              ║
    ║              C L O U D   A P I   v0.1.0                      ║
    ║         Pre-Execution Risk Scoring (PERS)                    ║
    ║                                                              ║
    ╚══════════════════════════════════════════════════════════════╝
    "#
    );
}
