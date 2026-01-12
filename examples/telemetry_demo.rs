//! Telemetry Export Demo
//!
//! Demonstrates the telemetry collection and marketing report generation
//!
//! Run with: cargo run --example telemetry_demo

use alloy_primitives::U256;
use ruster_revm::{TelemetryCollector, TelemetryEvent, ThreatType};
use std::sync::Arc;

fn main() {
    println!(
        r#"
    ╔══════════════════════════════════════════════════════════════╗
    ║                                                              ║
    ║   📊 TELEMETRY EXPORT DEMO                                   ║
    ║   Marketing Data Collection & Report Generation              ║
    ║                                                              ║
    ╚══════════════════════════════════════════════════════════════╝
    "#
    );

    // Create telemetry collector
    let collector = Arc::new(TelemetryCollector::new());

    println!("🔧 Simulating threat detection events...\n");

    // ============================================
    // SIMULATE VARIOUS THREAT DETECTIONS
    // ============================================

    // Simulate 50 honeypot detections
    for i in 0..50 {
        let value = U256::from((1 + i % 10) as u128 * 100_000_000_000_000_000u128); // 0.1-1 ETH
        let event = TelemetryEvent::new(
            ThreatType::Honeypot,
            value,
            15 + (i % 20) as u64, // 15-35ms latency
            5,                    // Critical
            format!("Sell failed: Token {} blocked transfers", i),
        );
        collector.record_threat(event);
    }
    println!("   ✅ Recorded 50 honeypot detections");

    // Simulate 100 high slippage warnings
    for i in 0..100 {
        let value = U256::from((5 + i % 20) as u128 * 100_000_000_000_000_000u128);
        let event = TelemetryEvent::new(
            ThreatType::HighSlippage,
            value,
            10 + (i % 15) as u64,
            3, // Medium
            format!("Slippage {}% detected", 5 + i % 10),
        );
        collector.record_threat(event);
    }
    println!("   ✅ Recorded 100 high slippage warnings");

    // Simulate 75 sandwich attack targets
    for i in 0..75 {
        let value = U256::from((2 + i % 15) as u128 * 1_000_000_000_000_000_000u128);
        let event = TelemetryEvent::new(
            ThreatType::SandwichTarget,
            value,
            20 + (i % 25) as u64,
            5, // Critical
            format!("Large swap with {}% slippage tolerance", 5 + i % 8),
        );
        collector.record_threat(event);
    }
    println!("   ✅ Recorded 75 sandwich attack targets");

    // Simulate 30 high tax tokens
    for i in 0..30 {
        let value = U256::from((1 + i % 5) as u128 * 500_000_000_000_000_000u128);
        let event = TelemetryEvent::new(
            ThreatType::HighTax,
            value,
            25 + (i % 20) as u64,
            4, // High
            format!("Token tax: {}%", 10 + i % 15),
        );
        collector.record_threat(event);
    }
    println!("   ✅ Recorded 30 high tax token warnings");

    // Simulate 500 clean analyses (no threats)
    for _ in 0..500 {
        collector.record_analysis(12);
    }
    println!("   ✅ Recorded 500 clean transaction analyses");

    println!();

    // ============================================
    // GENERATE STATISTICS
    // ============================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📊 CURRENT STATISTICS");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let stats = collector.get_stats();
    println!();
    println!("   Total Analyzed:        {:>10}", stats.total_analyzed);
    println!("   Total Threats:         {:>10}", stats.total_threats);
    println!("   Honeypots Detected:    {:>10}", stats.honeypots_detected);
    println!(
        "   Value Protected:       {:>10.2} ETH",
        stats.total_value_protected_eth
    );
    println!("   Avg Latency:           {:>10.2}ms", stats.avg_latency_ms);
    println!();
    println!("   Threats by Type:");
    for (threat_type, count) in &stats.threats_by_type {
        println!("     - {:<20} {:>5}", threat_type, count);
    }
    println!();

    // ============================================
    // GENERATE MARKETING REPORT
    // ============================================
    let eth_price = 2500.0; // Current ETH price in USD

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📈 MARKETING REPORT (ETH @ ${})", eth_price);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    println!("{}", collector.generate_marketing_report(eth_price));

    // ============================================
    // GENERATE SOCIAL MEDIA POST
    // ============================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📱 SOCIAL MEDIA POST (Discord/Telegram)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Generate marketing report directly from collector
    println!("{}", collector.generate_marketing_report(eth_price));
    println!();

    // ============================================
    // EXPORT DATA
    // ============================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("💾 DATA EXPORT");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();

    // Export JSON
    match collector.export_stats_json() {
        Ok(path) => println!("   ✅ JSON exported to: {}", path.display()),
        Err(e) => println!("   ❌ JSON export failed: {}", e),
    }

    // Export CSV
    match collector.export_stats_csv() {
        Ok(path) => println!("   ✅ CSV exported to: {}", path.display()),
        Err(e) => println!("   ❌ CSV export failed: {}", e),
    }

    println!();

    // ============================================
    // JSON API RESPONSE EXAMPLE
    // ============================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🔌 API RESPONSE FORMAT");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
    println!("GET /api/v1/telemetry/stats");
    println!();
    println!("{}", stats.to_json());
    println!();

    // ============================================
    // SUMMARY
    // ============================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("✅ DEMO COMPLETE");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
    println!("   Files created in ./telemetry/:");
    println!("   - stats_*.json     (Full statistics snapshot)");
    println!("   - telemetry_history.csv (Append-only history)");
    println!();
    println!("   Use these files for:");
    println!("   - Marketing dashboards");
    println!("   - Weekly reports to investors");
    println!("   - Social media content");
    println!("   - API endpoints for premium clients");
    println!();
}
