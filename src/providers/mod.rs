//! Providers Module - External Data Sources
//!
//! Jalur data: RPC clients, DexScreener, Alchemy Enhanced APIs, dan future integrations.
//! CEO Directive: Solana support placeholder included.
//!
//! Alchemy Best Practices Implemented:
//! - Gzip compression for 75% speedup
//! - Batch requests (max 50 per batch)
//! - Exponential backoff with jitter (1s→64s)
//! - Concurrent request handling

pub mod alchemy;
pub mod dexscreener;
pub mod rpc;

// Future: Solana Yellowstone gRPC support
// pub mod yellowstone;

// Future: WebSocket subscriptions
// pub mod websocket;

pub use alchemy::*;
pub use dexscreener::*;
pub use rpc::*;
