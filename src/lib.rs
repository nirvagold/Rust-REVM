//! Ruster REVM Library
//!
//! High-performance REVM-based token risk analyzer implementing
//! Pre-Execution Risk Scoring (PERS) for detecting:
//! - Honeypot tokens via simulated Buy-Approve-Sell cycles
//! - High tax tokens (fee-on-transfer)
//! - Sandwich attack targets
//! - MEV exposure risks
//!
//! # Architecture (Post Clean Sweep)
//!
//! ```text
//! src/
//! ├── core/           # Business logic: REVM Engine, Honeypot Detection
//! ├── providers/      # Data sources: RPC, DexScreener
//! ├── api/            # REST API: Axum handlers, routes, middleware
//! ├── utils/          # Helpers: Cache, Decoder, Telemetry, Constants
//! ├── models/         # Data structures: Types, Config, Errors
//! ├── main.rs         # CLI entry point
//! └── lib.rs          # Module exports (this file)
//! ```
//!
//! CEO Executive Order: Multi-Chain Alchemy Integration
//! - Dynamic URL construction from single ALCHEMY_API_KEY
//! - Multi-tier RPC fallback for resilience
//! - Exponential backoff retry logic
//! - Prepared for Solana support

// ============================================
// MODULE DECLARATIONS
// ============================================

pub mod api;
pub mod core;
pub mod models;
pub mod providers;
pub mod utils;

// ============================================
// RE-EXPORTS FOR BACKWARD COMPATIBILITY
// CEO Directive: Existing code must not break!
// ============================================

// Core exports
pub use core::analyzer::MempoolAnalyzer;
pub use core::honeypot::{HoneypotDetector, HoneypotResult, TokenInfo};
pub use core::risk_score::{RiskComponents, RiskScore, RiskScoreBuilder};
pub use core::simulator::Simulator;

// Models exports
pub use models::config::{ChainConfig, ChainId, DexRouters, SentryConfig};
pub use models::errors::{AppError, AppResult, ErrorCode};
pub use models::types::{AnalysisResult, RiskFactor, RiskLevel, SwapParams};

// Providers exports
pub use providers::dexscreener::{AutoDetectedToken, DexPair, DexScreenerClient, DiscoveredDex};
pub use providers::rpc::{AlchemyNetwork, RpcManager, RpcProvider};

// Utils exports
pub use utils::cache::{CacheStats, HoneypotCache};
pub use utils::constants::*;
pub use utils::decoder::SwapDecoder;
pub use utils::telemetry::{TelemetryCollector, TelemetryEvent, TelemetryStats, ThreatType};
