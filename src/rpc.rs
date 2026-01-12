//! RPC Client Module - Multi-Chain Alchemy Integration
//!
//! CEO Executive Order Implementation:
//! 1. Dynamic URL Construction from ALCHEMY_API_KEY
//! 2. Multi-tier RPC with fallback to public RPCs
//! 3. Exponential backoff retry logic
//! 4. User-Agent header & API key protection
//! 5. Modular architecture for future Solana support

use eyre::{eyre, Result};
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use serde::Deserialize;
use std::time::Duration;
use tracing::{info, warn};

/// RusterShield User-Agent for Alchemy dashboard monitoring
const USER_AGENT_STRING: &str = "RusterShield/1.0.0";

/// Default timeout for RPC requests (10 seconds per CEO directive)
const DEFAULT_TIMEOUT_SECS: u64 = 10;

/// Maximum retry attempts
const MAX_RETRIES: u32 = 3;

/// Base delay for exponential backoff (milliseconds)
const BASE_RETRY_DELAY_MS: u64 = 100;

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
    /// Get Alchemy subdomain for this network
    pub fn subdomain(&self) -> &'static str {
        match self {
            Self::EthMainnet => "eth-mainnet",
            Self::BscMainnet => "bnb-mainnet",
            Self::PolygonMainnet => "polygon-mainnet",
            Self::ArbitrumMainnet => "arb-mainnet",
            Self::OptimismMainnet => "opt-mainnet",
            Self::AvalancheMainnet => "avax-mainnet",
            Self::BaseMainnet => "base-mainnet",
            Self::SolanaMainnet => "solana-mainnet",
        }
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
            Self::SolanaMainnet => 0, // Non-EVM
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

/// Public RPC fallback URLs (last resort)
pub struct PublicRpcFallback;

impl PublicRpcFallback {
    pub fn get(chain_id: u64) -> Option<&'static str> {
        match chain_id {
            1 => Some("https://eth.llamarpc.com"),
            56 => Some("https://bsc-dataseed.binance.org"),
            137 => Some("https://polygon-rpc.com"),
            42161 => Some("https://arb1.arbitrum.io/rpc"),
            10 => Some("https://mainnet.optimism.io"),
            43114 => Some("https://api.avax.network/ext/bc/C/rpc"),
            8453 => Some("https://mainnet.base.org"),
            _ => None,
        }
    }
}

/// RPC Provider with retry logic and fallback support
#[derive(Clone)]
pub struct RpcProvider {
    /// Primary RPC URL (Alchemy)
    primary_url: String,
    /// Fallback RPC URL (public)
    fallback_url: Option<String>,
    /// HTTP client with custom headers
    client: reqwest::Client,
    /// Chain ID for this provider
    chain_id: u64,
    /// Network name for logging
    network_name: String,
}

impl RpcProvider {
    /// Create a new RPC provider for a chain
    /// Dynamically constructs Alchemy URL from ALCHEMY_API_KEY
    pub fn new(chain_id: u64) -> Result<Self> {
        let network = AlchemyNetwork::from_chain_id(chain_id)
            .ok_or_else(|| eyre!("Unsupported chain_id: {}", chain_id))?;

        let api_key = Self::get_alchemy_key()?;
        let primary_url = Self::build_alchemy_url(&network, &api_key);
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
            fallback_url: None, // No public Solana fallback
            client,
            chain_id: 0,
            network_name: "solana-mainnet".to_string(),
        })
    }

    /// Get Alchemy API key from environment
    /// NEVER logs the actual key (CEO security directive)
    fn get_alchemy_key() -> Result<String> {
        // Try dedicated ALCHEMY_API_KEY first
        if let Ok(key) = std::env::var("ALCHEMY_API_KEY") {
            if !key.is_empty() && key != "YOUR_API_KEY" {
                info!("🔑 Using ALCHEMY_API_KEY (key hidden)");
                return Ok(key);
            }
        }

        // Fallback: extract from ETH_HTTP_URL
        if let Ok(url) = std::env::var("ETH_HTTP_URL") {
            if let Some(key) = url.split("/v2/").nth(1) {
                if !key.is_empty() && key != "YOUR_API_KEY" {
                    info!("🔑 Extracted API key from ETH_HTTP_URL (key hidden)");
                    return Ok(key.to_string());
                }
            }
        }

        Err(eyre!("ALCHEMY_API_KEY not configured. Set ALCHEMY_API_KEY environment variable."))
    }

    /// Build Alchemy URL dynamically
    fn build_alchemy_url(network: &AlchemyNetwork, api_key: &str) -> String {
        format!("https://{}.g.alchemy.com/v2/{}", network.subdomain(), api_key)
    }

    /// Build HTTP client with custom headers
    fn build_client() -> Result<reqwest::Client> {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(USER_AGENT_STRING),
        );
        headers.insert(
            "Content-Type",
            HeaderValue::from_static("application/json"),
        );

        reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
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

    /// Execute call with exponential backoff retry
    async fn call_with_retry<T: for<'de> Deserialize<'de>>(
        &self,
        url: &str,
        payload: &serde_json::Value,
    ) -> Result<T> {
        let mut last_error = None;

        for attempt in 0..MAX_RETRIES {
            if attempt > 0 {
                // Exponential backoff: 100ms, 200ms, 400ms...
                let delay = BASE_RETRY_DELAY_MS * (2_u64.pow(attempt - 1));
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }

            match self.execute_call::<T>(url, payload).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    // Check for rate limit (HTTP 429)
                    if e.to_string().contains("429") || e.to_string().contains("rate limit") {
                        warn!("⏳ Rate limited, backing off (attempt {}/{})", attempt + 1, MAX_RETRIES);
                    }
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| eyre!("Unknown error after {} retries", MAX_RETRIES)))
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

        // Check HTTP status
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
        let params = serde_json::json!([
            {
                "to": to,
                "data": data
            },
            "latest"
        ]);

        self.call::<String>("eth_call", params).await
    }

    /// Get bytecode (EVM chains only)
    pub async fn get_code(&self, address: &str) -> Result<String> {
        let params = serde_json::json!([address, "latest"]);
        self.call::<String>("eth_getCode", params).await
    }

    /// Get RPC URL (for logging, masks API key)
    pub fn masked_url(&self) -> String {
        // Mask API key in URL for safe logging
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
#[derive(Debug, Deserialize)]
struct RpcError {
    code: i64,
    message: String,
}

/// Multi-chain RPC manager
/// Manages providers for all supported chains
pub struct RpcManager {
    providers: std::collections::HashMap<u64, RpcProvider>,
    solana_provider: Option<RpcProvider>,
}

impl RpcManager {
    /// Create manager with all supported chains
    pub fn new() -> Self {
        let mut providers = std::collections::HashMap::new();

        // Initialize EVM chain providers
        for chain_id in [1, 56, 137, 42161, 10, 43114, 8453] {
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

        // Initialize Solana provider
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

        Self {
            providers,
            solana_provider,
        }
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
        assert_eq!(AlchemyNetwork::SolanaMainnet.subdomain(), "solana-mainnet");
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
}
