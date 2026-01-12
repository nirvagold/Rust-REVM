//! Ruster REVM - High-performance REVM-based token risk analyzer
//!
//! Pre-Execution Risk Scoring (PERS) engine that detects:
//! - Honeypot tokens via simulated Buy-Approve-Sell cycles
//! - High tax tokens (fee-on-transfer)
//! - Sandwich attack targets
//! - MEV exposure risks
//!
//! CEO Directive: Uses new modular architecture

// Import from library (new structure)
use ruster_revm::{MempoolAnalyzer, SentryConfig, TelemetryCollector};

use eyre::Result;
use std::sync::Arc;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .compact()
        .init();

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
    ║              P E R S   E n g i n e   v0.1.0                  ║
    ║         Pre-Execution Risk Scoring System                    ║
    ║                                                              ║
    ╚══════════════════════════════════════════════════════════════╝
    "#
    );

    // Check for required environment variables
    let wss_url = std::env::var("ETH_WSS_URL");
    let _http_url = std::env::var("ETH_HTTP_URL");

    if wss_url.is_err() {
        eprintln!("⚠️  WARNING: ETH_WSS_URL not set!");
        eprintln!("   Please set your Alchemy/QuickNode WebSocket URL:");
        eprintln!("   $env:ETH_WSS_URL = \"wss://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY\"");
        eprintln!();
    }

    // Load configuration
    let config = SentryConfig::default();

    // Initialize telemetry collector
    let telemetry = Arc::new(TelemetryCollector::new());
    println!("📊 Telemetry initialized. Data will be exported to ./telemetry/");

    // Create and run analyzer
    let analyzer = MempoolAnalyzer::new(config, telemetry.clone());

    // Run with graceful shutdown on Ctrl+C
    tokio::select! {
        result = analyzer.run() => {
            if let Err(e) = result {
                eprintln!("❌ Error: {}", e);
                return Err(e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("\n\n🛑 Shutting down gracefully...");

            // Print final statistics
            let stats = analyzer.get_stats();
            println!("\n📊 Final Statistics:");
            println!("   Total Received:  {}", stats.total_received);
            println!("   Total Filtered:  {}", stats.total_filtered);
            println!("   Total Analyzed:  {}", stats.total_analyzed);
            println!("   Total Risky:     {}", stats.total_risky);
            println!("   Avg Latency:     {:.2}ms", stats.avg_latency_ms);

            // Export telemetry
            println!("\n📈 Exporting telemetry data...");

            // Generate marketing report (assume $2500/ETH for demo)
            let eth_price = 2500.0;
            println!("{}", telemetry.generate_marketing_report(eth_price));

            // Export to files
            match telemetry.export_stats_json() {
                Ok(path) => println!("   ✅ JSON exported to: {}", path.display()),
                Err(e) => println!("   ❌ JSON export failed: {}", e),
            }

            match telemetry.export_stats_csv() {
                Ok(path) => println!("   ✅ CSV exported to: {}", path.display()),
                Err(e) => println!("   ❌ CSV export failed: {}", e),
            }
        }
    }

    Ok(())
}
