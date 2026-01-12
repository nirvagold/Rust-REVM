//! Honeypot Detection Demo
//!
//! Demonstrates the Buy → Sell simulation to detect honeypot tokens
//!
//! Run with: cargo run --example honeypot_demo

use alloy_primitives::{Address, U256};
use ruster_revm::{HoneypotDetector, HoneypotResult};
use std::time::Instant;

fn main() {
    println!(
        r#"
    ╔══════════════════════════════════════════════════════════════╗
    ║                                                              ║
    ║   🍯 HONEYPOT DETECTOR DEMO                                  ║
    ║   Simulates Buy → Sell cycle to detect honeypot tokens       ║
    ║                                                              ║
    ╚══════════════════════════════════════════════════════════════╝
    "#
    );

    // Create detector for Ethereum mainnet
    let detector = HoneypotDetector::mainnet();

    // Test amount: 0.1 ETH
    let test_amount = U256::from(100_000_000_000_000_000u128);

    println!("🔬 Test Configuration:");
    println!("   Chain: Ethereum Mainnet (ID: 1)");
    println!("   Test Amount: 0.1 ETH");
    println!("   Router: Uniswap V2");
    println!();

    // ============================================
    // TEST CASE 1: Known Safe Token (simulated)
    // ============================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📋 TEST 1: Simulated Safe Token");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let safe_token: Address = "0x1111111111111111111111111111111111111111"
        .parse()
        .unwrap();

    let start = Instant::now();
    let result = detector.detect(safe_token, test_amount, None, None, None, None);
    let elapsed = start.elapsed();

    match result {
        Ok(hp_result) => {
            println!("   Token: {}", safe_token);
            println!("   {}", hp_result.summary());
            println!("   Detection Time: {:?}", elapsed);
        }
        Err(e) => {
            println!("   ❌ Error: {}", e);
        }
    }
    println!();

    // ============================================
    // TEST CASE 2: Multiple Token Addresses
    // ============================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📋 TEST 2: Batch Token Analysis");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let test_tokens = vec![
        ("Token A", "0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
        ("Token B", "0xBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB"),
        ("Token C", "0xCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC"),
    ];

    let batch_start = Instant::now();

    for (name, addr_str) in test_tokens {
        let token: Address = addr_str.parse().unwrap();
        let start = Instant::now();

        match detector.detect(token, test_amount, None, None, None, None) {
            Ok(hp_result) => {
                let status = if hp_result.is_honeypot {
                    "🚨 HONEYPOT"
                } else {
                    "✅ SAFE"
                };
                println!(
                    "   {} | {} | Latency: {}ms",
                    name,
                    status,
                    start.elapsed().as_millis()
                );
            }
            Err(e) => {
                println!("   {} | ❌ Error: {}", name, e);
            }
        }
    }

    println!();
    println!("   Total Batch Time: {:?}", batch_start.elapsed());
    println!();

    // ============================================
    // PERFORMANCE BENCHMARK
    // ============================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📊 PERFORMANCE BENCHMARK (100 iterations)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let benchmark_token: Address = "0xDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD"
        .parse()
        .unwrap();
    let iterations = 100;

    let bench_start = Instant::now();
    let mut total_latency_ms = 0u128;
    let mut min_latency_ms = u128::MAX;
    let mut max_latency_ms = 0u128;

    for _ in 0..iterations {
        let iter_start = Instant::now();
        let _ = detector.detect(benchmark_token, test_amount, None, None, None, None);
        let latency = iter_start.elapsed().as_millis();

        total_latency_ms += latency;
        min_latency_ms = min_latency_ms.min(latency);
        max_latency_ms = max_latency_ms.max(latency);
    }

    let avg_latency = total_latency_ms as f64 / iterations as f64;
    let total_time = bench_start.elapsed();

    println!("   Iterations: {}", iterations);
    println!("   Total Time: {:?}", total_time);
    println!("   Avg Latency: {:.2}ms", avg_latency);
    println!("   Min Latency: {}ms", min_latency_ms);
    println!("   Max Latency: {}ms", max_latency_ms);
    println!(
        "   Throughput: {:.0} checks/sec",
        iterations as f64 / total_time.as_secs_f64()
    );
    println!();

    // ============================================
    // EXPECTED OUTPUT FORMAT
    // ============================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📋 EXPECTED OUTPUT IN PRODUCTION");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Simulate honeypot detection output
    let honeypot_example = HoneypotResult::honeypot(
        "Sell failed: Reverted - transfer blocked".to_string(),
        true,
        false,
        true,
        50,
        vec!["setBots detected".to_string()],
        23,
    );

    println!();
    println!("   Example Honeypot Detection:");
    println!("   {}", honeypot_example.summary());
    println!();

    let safe_example = HoneypotResult::safe(2.5, 3.0, 0, vec![], 18);
    println!("   Example Safe Token:");
    println!("   {}", safe_example.summary());
    println!();

    // ============================================
    // INTEGRATION EXAMPLE
    // ============================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("💡 INTEGRATION WITH MEMPOOL ANALYZER");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
    println!("   When a swap transaction is detected:");
    println!("   1. Extract token address from swap path");
    println!("   2. Run honeypot detection (Buy → Sell simulation)");
    println!("   3. If honeypot detected → CRITICAL risk");
    println!("   4. If high tax detected → HIGH risk");
    println!();
    println!("   Expected output in analyzer:");
    println!();
    println!("   💀 Risk: CRITICAL | TX: 0x1234abcd...");
    println!("      From: 0xUser...");
    println!("      To: 0x7a250d56... (Uniswap V2)");
    println!("      Latency: 45ms");
    println!("      Factors:");
    println!("        - 🚨 HONEYPOT: Sell failed: Reverted | Buy: ✅ | Sell: ❌");
    println!();

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("✅ DEMO COMPLETE");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
}
