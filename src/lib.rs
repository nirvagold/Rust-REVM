//! Ruster REVM Library
//!
//! High-performance REVM-based token risk analyzer implementing
//! Pre-Execution Risk Scoring (PERS) for detecting:
//! - Honeypot tokens via simulated Buy-Approve-Sell cycles
//! - High tax tokens (fee-on-transfer)
//! - Sandwich attack targets
//! - MEV exposure risks
//!
//! CEO Executive Order: Multi-Chain Alchemy Integration
//! - Dynamic URL construction from single ALCHEMY_API_KEY
//! - Multi-tier RPC fallback for resilience
//! - Exponential backoff retry logic
//! - Prepared for Solana support

pub mod analyzer;
pub mod api;
pub mod cache;
pub mod config;
pub mod decoder;
pub mod dexscreener;
pub mod honeypot;
pub mod risk_score;
pub mod rpc;
pub mod simulator;
pub mod telemetry;
pub mod types;

pub use analyzer::MempoolAnalyzer;
pub use cache::{CacheStats, HoneypotCache};
pub use config::{ChainConfig, ChainId, DexRouters, SentryConfig};
pub use decoder::SwapDecoder;
pub use dexscreener::{DexScreenerClient, DexPair, DiscoveredDex, AutoDetectedToken};
pub use honeypot::{HoneypotDetector, HoneypotResult, TokenInfo};
pub use risk_score::{RiskComponents, RiskScore, RiskScoreBuilder};
pub use rpc::{RpcProvider, RpcManager, AlchemyNetwork};
pub use simulator::Simulator;
pub use telemetry::{TelemetryCollector, TelemetryEvent, TelemetryStats, ThreatType};
pub use types::{AnalysisResult, RiskFactor, RiskLevel, SwapParams};
