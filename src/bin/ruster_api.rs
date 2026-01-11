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

use ruster_revm::api::{create_router, handlers::AppState};
use ruster_revm::telemetry::TelemetryCollector;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Initialize logging
    let _subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .compact()
        .init();
    
    print_banner();
    
    // Initialize telemetry
    let telemetry = Arc::new(TelemetryCollector::new());
    
    // Create app state
    let state = Arc::new(AppState::new(telemetry));
    
    // Create router
    let app = create_router(state);
    
    // Get server config from env
    let host = std::env::var("RUSTER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port: u16 = std::env::var("RUSTER_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    
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
    
    // Start server
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}

fn print_banner() {
    println!(r#"
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
    "#);
}
