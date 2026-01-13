//! RPC Client Module - Multi-Chain Alchemy Integration
//!
//! CEO Executive Order Implementation:
//! 1. Dynamic URL Construction from ALCHEMY_API_KEY
//! 2. Multi-tier RPC with fallback to public RPCs
//! 3. Exponential backoff retry logic (Alchemy Best Practice: 1s→2s→4s→8s→...→64s with jitter)
//! 4. User-Agent header & API key protection
//! 5. Modular architecture for future Solana support
//! 6. Gzip compression for 75% speedup on large responses (Alchemy Best Practice)
//! 7. Batch requests support (max 50 per batch - Alchemy Best Practice)
//! 8. Concurrent request handling with tokio::spawn
//!
//! Alchemy Documentation Reference:
//! - Compression: https://alchemy.com/docs/how-to-enable-compression-to-speed-up-json-rpc-blockchain-requests.mdx
//! - Batch Requests: https://alchemy.com/docs/reference/batch-requests.mdx
//! - Throughput & 429: https://alchemy.com/docs/reference/throughput.mdx
//! - Retries: https://alchemy.com/docs/how-to-implement-retries.mdx
//!
//! CEO Directive: Uses constants from utils/constants.rs

use eyre::{eyre, Result};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT_ENCODING, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info, warn};
use rand::Rng;

use crate::utils::constants::{
    build_alchemy_url, get_alchemy_subdomain, get_public_rpc_fallback,
    DEFAULT_RPC_TIMEOUT_SECS, SUPPORTED_CHAIN_IDS, USER_AGENT as USER_AGENT_CONST,
};

// ============================================
// ALCHEMY BEST PRACTICE CONSTANTS
// ============================================

/// Maximum batch size (Alchemy recommends 50 for reliability, NOT 1000)
pub const MAX_BATCH_SIZE: usize = 50;

/// Base retry delay in milliseconds (Alchemy: start at 1000ms)
pub const ALCHEMY_BASE_RETRY_MS: u64 = 1000;

/// Maximum retry delay in milliseconds (Alchemy: cap at 64 seconds)
pub const ALCHEMY_MAX_RETRY_MS: u64 = 64000;

/// Maximum retry attempts (Alchemy: exponential backoff 1s→2s→4s→8s→16s→32s→64s = 7 attempts)
pub const ALCHEMY_MAX_RETRIES: u32 = 7;

/// Jitter percentage for retry delay (Alchemy: add random jitter to prevent thundering herd)
pub const RETRY_JITTER_PERCENT: u64 = 20;

/// Alchemy network identifiers for dynamic URL construction
#[derive(Debug, Clone, Copy)]
pub enum AlchemyNetwork {
    EthMainnet,
    BscMainnet,
    PolygonMainnet,
    ArbitrumMainnet,
    OptimismMainnet,
    AvalancheMainnet,
    BaseMainnet,
    SolanaMainnet,
}

impl AlchemyNetwork {
    /// Get Alchemy subdomain for this network (delegates to constants)
    pub fn subdomain(&self) -> &'static str {
        get_alchemy_subdomain(self.chain_id()).unwrap_or("eth-mainnet")
    }

    /// Get chain ID (0 for non-EVM chains like Solana)
    pub fn chain_id(&self) -> u64 {
        match self {
            Self::EthMainnet => 1,
            Self::BscMainnet => 56,
            Self::PolygonMainnet => 137,
            Self::ArbitrumMainnet => 42161,
            Self::OptimismMainnet => 10,
            Self::AvalancheMainnet => 43114,
            Self::BaseMainnet => 8453,
            Self::SolanaMainnet => 0,
        }
    }

    /// Get network from chain ID
    pub fn from_chain_id(chain_id: u64) -> Option<Self> {
        match chain_id {
            1 => Some(Self::EthMainnet),
            56 => Some(Self::BscMainnet),
            137 => Some(Self::PolygonMainnet),
            42161 => Some(Self::ArbitrumMainnet),
            10 => Some(Self::OptimismMainnet),
            43114 => Some(Self::AvalancheMainnet),
            8453 => Some(Self::BaseMainnet),
            _ => None,
        }
    }

    /// Check if this is an EVM chain
    pub fn is_evm(&self) -> bool {
        !matches!(self, Self::SolanaMainnet)
    }
}

/// Public RPC fallback URLs (delegates to constants)
pub struct PublicRpcFallback;

impl PublicRpcFallback {
    pub fn get(chain_id: u64) -> Option<&'static str> {
        get_public_rpc_fallback(chain_id)
    }
}

/// Batch JSON-RPC request item
#[derive(Debug, Clone, Serialize)]
pub struct BatchRequestItem {
    pub jsonrpc: &'static str,
    pub method: String,
    pub params: serde_json::Value,
    pub id: u64,
}

/// Batch JSON-RPC response item
#[derive(Debug, Clone, Deserialize)]
pub struct BatchResponseItem<T> {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub result: Option<T>,
    pub error: Option<RpcError>,
    pub id: u64,
}

/// RPC Provider with retry logic, fallback support, and Alchemy best practices
#[derive(Clone)]
pub struct RpcProvider {
    /// Primary RPC URL (Alchemy)
    primary_url: String,
    /// Fallback RPC URL (public)
    fallback_url: Option<String>,
    /// HTTP client with custom headers (gzip enabled)
    client: reqwest::Client,
    /// Chain ID for this provider
    chain_id: u64,
    /// Network name for logging
    network_name: String,
}

impl RpcProvider {
    /// Create a new RPC provider for a chain
    pub fn new(chain_id: u64) -> Result<Self> {
        let network = AlchemyNetwork::from_chain_id(chain_id)
            .ok_or_else(|| eyre!("Unsupported chain_id: {}", chain_id))?;

        let api_key = Self::get_alchemy_key()?;
        let primary_url = build_alchemy_url(chain_id, &api_key)
            .ok_or_else(|| eyre!("Cannot build Alchemy URL for chain {}", chain_id))?;
        let fallback_url = PublicRpcFallback::get(chain_id).map(String::from);

        let client = Self::build_client()?;

        Ok(Self {
            primary_url,
            fallback_url,
            client,
            chain_id,
            network_name: network.subdomain().to_string(),
        })
    }

    /// Create provider for Solana
    pub fn solana() -> Result<Self> {
        let api_key = Self::get_alchemy_key()?;
        let primary_url = format!("https://solana-mainnet.g.alchemy.com/v2/{}", api_key);

        let client = Self::build_client()?;

        Ok(Self {
            primary_url,
            fallback_url: None,
            client,
            chain_id: 0,
            network_name: "solana-mainnet".to_string(),
        })
    }

    /// Get Alchemy API key from environment
    fn get_alchemy_key() -> Result<String> {
        if let Ok(key) = std::env::var("ALCHEMY_API_KEY") {
            if !key.is_empty() && key != "YOUR_API_KEY" {
                info!("🔑 Using ALCHEMY_API_KEY (key hidden)");
                return Ok(key);
            }
        }

        if let Ok(url) = std::env::var("ETH_HTTP_URL") {
            if let Some(key) = url.split("/v2/").nth(1) {
                if !key.is_empty() && key != "YOUR_API_KEY" {
                    info!("🔑 Extracted API key from ETH_HTTP_URL (key hidden)");
                    return Ok(key.to_string());
                }
            }
        }

        Err(eyre!("ALCHEMY_API_KEY not configured"))
    }

    /// Build HTTP client with custom headers (Alchemy Best Practice: gzip compression)
    fn build_client() -> Result<reqwest::Client> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_CONST));
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));
        // Alchemy Best Practice: Enable gzip compression for 75% speedup on responses >100kb
        headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("gzip"));

        reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(DEFAULT_RPC_TIMEOUT_SECS))
            .gzip(true) // Enable automatic gzip decompression
            .build()
            .map_err(|e| eyre!("Failed to build HTTP client: {}", e))
    }

    /// Execute JSON-RPC call with retry logic and fallback
    pub async fn call<T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<T> {
        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": 1
        });

        // Try primary (Alchemy) with retries
        match self.call_with_retry(&self.primary_url, &payload).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                warn!("⚠️ Primary RPC failed on {}: {}", self.network_name, e);
            }
        }

        // Try fallback if available
        if let Some(ref fallback) = self.fallback_url {
            info!("🔄 Trying fallback RPC for {}", self.network_name);
            match self.call_with_retry(fallback, &payload).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    warn!("⚠️ Fallback RPC also failed: {}", e);
                }
            }
        }

        Err(eyre!("All RPC endpoints failed for {}", self.network_name))
    }

    /// Execute call with Alchemy-recommended exponential backoff (1s→2s→4s→...→64s with jitter)
    async fn call_with_retry<T: for<'de> Deserialize<'de>>(
        &self,
        url: &str,
        payload: &serde_json::Value,
    ) -> Result<T> {
        let mut last_error = None;

        for attempt in 0..ALCHEMY_MAX_RETRIES {
            if attempt > 0 {
                // Alchemy Best Practice: Exponential backoff with jitter
                let base_delay = ALCHEMY_BASE_RETRY_MS * (2_u64.pow(attempt - 1));
                let capped_delay = base_delay.min(ALCHEMY_MAX_RETRY_MS);
                
                // Add random jitter (±20%) to prevent thundering herd
                let jitter_range = (capped_delay * RETRY_JITTER_PERCENT) / 100;
                let jitter: i64 = rand::thread_rng().gen_range(-(jitter_range as i64)..=(jitter_range as i64));
                let final_delay = (capped_delay as i64 + jitter).max(100) as u64;
                
                debug!("⏳ Retry {}/{} after {}ms (base: {}ms, jitter: {}ms)", 
                    attempt + 1, ALCHEMY_MAX_RETRIES, final_delay, capped_delay, jitter);
                tokio::time::sleep(Duration::from_millis(final_delay)).await;
            }

            match self.execute_call::<T>(url, payload).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    if e.to_string().contains("429") || e.to_string().contains("rate limit") {
                        warn!("⏳ Rate limited (HTTP 429), backing off (attempt {}/{})", 
                            attempt + 1, ALCHEMY_MAX_RETRIES);
                    }
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| eyre!("Unknown error after {} retries", ALCHEMY_MAX_RETRIES)))
    }

    /// Execute single RPC call
    async fn execute_call<T: for<'de> Deserialize<'de>>(
        &self,
        url: &str,
        payload: &serde_json::Value,
    ) -> Result<T> {
        let response = self.client
            .post(url)
            .json(payload)
            .send()
            .await
            .map_err(|e| eyre!("Request failed: {}", e))?;

        let status = response.status();
        if status == 429 {
            return Err(eyre!("Rate limited (HTTP 429)"));
        }
        if !status.is_success() {
            return Err(eyre!("HTTP error: {}", status));
        }

        let json: RpcResponse<T> = response.json().await
            .map_err(|e| eyre!("Failed to parse response: {}", e))?;

        if let Some(error) = json.error {
            return Err(eyre!("RPC error: {} (code: {})", error.message, error.code));
        }

        json.result.ok_or_else(|| eyre!("No result in response"))
    }

    /// Execute eth_call (EVM chains only)
    pub async fn eth_call(&self, to: &str, data: &str) -> Result<String> {
        let params = serde_json::json!([{ "to": to, "data": data }, "latest"]);
        self.call::<String>("eth_call", params).await
    }

    /// Get bytecode (EVM chains only)
    pub async fn get_code(&self, address: &str) -> Result<String> {
        let params = serde_json::json!([address, "latest"]);
        self.call::<String>("eth_getCode", params).await
    }

    /// Get RPC URL (masked for logging)
    pub fn masked_url(&self) -> String {
        if self.primary_url.contains("/v2/") {
            let parts: Vec<&str> = self.primary_url.split("/v2/").collect();
            if parts.len() == 2 {
                return format!("{}/v2/***HIDDEN***", parts[0]);
            }
        }
        self.primary_url.clone()
    }

    /// Get chain ID
    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }

    // ============================================
    // ALCHEMY BEST PRACTICE: BATCH REQUESTS
    // ============================================

    /// Execute batch JSON-RPC calls (Alchemy Best Practice: max 50 per batch)
    /// 
    /// Reference: https://alchemy.com/docs/reference/batch-requests.mdx
    /// - Maximum 50 requests per batch for reliability
    /// - All requests in batch share same retry logic
    pub async fn batch_call<T: for<'de> Deserialize<'de> + Clone>(
        &self,
        requests: Vec<(&str, serde_json::Value)>,
    ) -> Result<Vec<Result<T>>> {
        if requests.is_empty() {
            return Ok(vec![]);
        }

        // Split into chunks of MAX_BATCH_SIZE (50)
        let chunks: Vec<_> = requests.chunks(MAX_BATCH_SIZE).collect();
        let mut all_results = Vec::with_capacity(requests.len());

        for chunk in chunks {
            let batch_payload: Vec<serde_json::Value> = chunk
                .iter()
                .enumerate()
                .map(|(idx, (method, params))| {
                    serde_json::json!({
                        "jsonrpc": "2.0",
                        "method": method,
                        "params": params,
                        "id": idx + 1
                    })
                })
                .collect();

            let results = self.execute_batch::<T>(&batch_payload).await?;
            all_results.extend(results);
        }

        Ok(all_results)
    }

    /// Execute batch request with retry
    async fn execute_batch<T: for<'de> Deserialize<'de> + Clone>(
        &self,
        batch_payload: &[serde_json::Value],
    ) -> Result<Vec<Result<T>>> {
        let mut last_error = None;

        for attempt in 0..ALCHEMY_MAX_RETRIES {
            if attempt > 0 {
                let base_delay = ALCHEMY_BASE_RETRY_MS * (2_u64.pow(attempt - 1));
                let capped_delay = base_delay.min(ALCHEMY_MAX_RETRY_MS);
                let jitter_range = (capped_delay * RETRY_JITTER_PERCENT) / 100;
                let jitter: i64 = rand::thread_rng().gen_range(-(jitter_range as i64)..=(jitter_range as i64));
                let final_delay = (capped_delay as i64 + jitter).max(100) as u64;
                
                tokio::time::sleep(Duration::from_millis(final_delay)).await;
            }

            let response = self.client
                .post(&self.primary_url)
                .json(batch_payload)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let status = resp.status();
                    if status == 429 {
                        last_error = Some(eyre!("Rate limited (HTTP 429)"));
                        continue;
                    }
                    if !status.is_success() {
                        last_error = Some(eyre!("HTTP error: {}", status));
                        continue;
                    }

                    let batch_response: Vec<BatchResponseItem<T>> = resp.json().await
                        .map_err(|e| eyre!("Failed to parse batch response: {}", e))?;

                    // Convert to Vec<Result<T>>
                    let results: Vec<Result<T>> = batch_response
                        .into_iter()
                        .map(|item| {
                            if let Some(error) = item.error {
                                Err(eyre!("RPC error: {} (code: {})", error.message, error.code))
                            } else if let Some(result) = item.result {
                                Ok(result)
                            } else {
                                Err(eyre!("No result in response for id {}", item.id))
                            }
                        })
                        .collect();

                    return Ok(results);
                }
                Err(e) => {
                    last_error = Some(eyre!("Request failed: {}", e));
                }
            }
        }

        Err(last_error.unwrap_or_else(|| eyre!("Batch request failed after {} retries", ALCHEMY_MAX_RETRIES)))
    }

    // ============================================
    // ALCHEMY BEST PRACTICE: CONCURRENT REQUESTS
    // ============================================

    /// Execute multiple calls concurrently (Alchemy Best Practice: parallel execution)
    /// 
    /// Reference: https://alchemy.com/docs/best-practices-when-using-alchemy.mdx
    /// - Treat Alchemy as multiple nodes, not single node
    /// - Use concurrent requests for better throughput
    pub async fn concurrent_calls<T: for<'de> Deserialize<'de> + Send + 'static>(
        &self,
        requests: Vec<(&str, serde_json::Value)>,
    ) -> Vec<Result<T>> {
        let futures: Vec<_> = requests
            .into_iter()
            .map(|(method, params)| {
                let provider = self.clone();
                let method = method.to_string();
                tokio::spawn(async move {
                    provider.call::<T>(&method, params).await
                })
            })
            .collect();

        let mut results = Vec::with_capacity(futures.len());
        for future in futures {
            match future.await {
                Ok(result) => results.push(result),
                Err(e) => results.push(Err(eyre!("Task join error: {}", e))),
            }
        }
        results
    }
}

/// JSON-RPC response structure
#[derive(Debug, Deserialize)]
struct RpcResponse<T> {
    #[allow(dead_code)]
    jsonrpc: String,
    result: Option<T>,
    error: Option<RpcError>,
    #[allow(dead_code)]
    id: u64,
}

/// JSON-RPC error structure
#[derive(Debug, Clone, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

impl RpcError {
    /// Check if this is a rate limit error (Alchemy: HTTP 429 or code -32005)
    pub fn is_rate_limit(&self) -> bool {
        self.code == -32005 || self.message.to_lowercase().contains("rate limit")
    }

    /// Check if this is a method not found error (code -32601)
    pub fn is_method_not_found(&self) -> bool {
        self.code == -32601
    }

    /// Check if this is a parse error (code -32700)
    pub fn is_parse_error(&self) -> bool {
        self.code == -32700
    }

    /// Check if this is an invalid request error (code -32600)
    pub fn is_invalid_request(&self) -> bool {
        self.code == -32600
    }
}

/// Multi-chain RPC manager
pub struct RpcManager {
    providers: std::collections::HashMap<u64, RpcProvider>,
    solana_provider: Option<RpcProvider>,
}

impl RpcManager {
    /// Create manager with all supported chains
    pub fn new() -> Self {
        let mut providers = std::collections::HashMap::new();

        for chain_id in SUPPORTED_CHAIN_IDS {
            match RpcProvider::new(chain_id) {
                Ok(provider) => {
                    info!("✅ Initialized RPC for chain {} ({})", chain_id, provider.masked_url());
                    providers.insert(chain_id, provider);
                }
                Err(e) => {
                    warn!("⚠️ Failed to initialize RPC for chain {}: {}", chain_id, e);
                }
            }
        }

        let solana_provider = match RpcProvider::solana() {
            Ok(provider) => {
                info!("✅ Initialized Solana RPC ({})", provider.masked_url());
                Some(provider)
            }
            Err(e) => {
                warn!("⚠️ Failed to initialize Solana RPC: {}", e);
                None
            }
        };

        Self { providers, solana_provider }
    }

    /// Get provider for a chain
    pub fn get(&self, chain_id: u64) -> Option<&RpcProvider> {
        self.providers.get(&chain_id)
    }

    /// Get Solana provider
    pub fn solana(&self) -> Option<&RpcProvider> {
        self.solana_provider.as_ref()
    }

    /// Check if chain is supported
    pub fn is_supported(&self, chain_id: u64) -> bool {
        self.providers.contains_key(&chain_id)
    }
}

impl Default for RpcManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alchemy_network_subdomain() {
        assert_eq!(AlchemyNetwork::EthMainnet.subdomain(), "eth-mainnet");
        assert_eq!(AlchemyNetwork::BaseMainnet.subdomain(), "base-mainnet");
    }

    #[test]
    fn test_chain_id_mapping() {
        assert_eq!(AlchemyNetwork::from_chain_id(1).unwrap().chain_id(), 1);
        assert_eq!(AlchemyNetwork::from_chain_id(8453).unwrap().chain_id(), 8453);
        assert!(AlchemyNetwork::from_chain_id(999).is_none());
    }

    #[test]
    fn test_public_fallback() {
        assert!(PublicRpcFallback::get(1).is_some());
        assert!(PublicRpcFallback::get(999).is_none());
    }

    #[test]
    fn test_alchemy_retry_constants() {
        // Verify Alchemy best practice constants
        assert_eq!(MAX_BATCH_SIZE, 50); // Alchemy recommends 50, not 1000
        assert_eq!(ALCHEMY_BASE_RETRY_MS, 1000); // Start at 1 second
        assert_eq!(ALCHEMY_MAX_RETRY_MS, 64000); // Cap at 64 seconds
        assert_eq!(ALCHEMY_MAX_RETRIES, 7); // 1s→2s→4s→8s→16s→32s→64s
    }

    #[test]
    fn test_rpc_error_classification() {
        let rate_limit_error = RpcError {
            code: -32005,
            message: "Rate limit exceeded".to_string(),
        };
        assert!(rate_limit_error.is_rate_limit());

        let method_not_found = RpcError {
            code: -32601,
            message: "Method not found".to_string(),
        };
        assert!(method_not_found.is_method_not_found());

        let parse_error = RpcError {
            code: -32700,
            message: "Parse error".to_string(),
        };
        assert!(parse_error.is_parse_error());
    }
}
