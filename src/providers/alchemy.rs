//! Alchemy Enhanced APIs Module
//!
//! Implements Alchemy-specific APIs that go beyond standard JSON-RPC:
//! 1. Token API - alchemy_getTokenMetadata, alchemy_getTokenBalances
//! 2. Transaction Simulation - alchemy_simulateAssetChanges
//! 3. Prices API - Token prices by address
//! 4. Transfers API - alchemy_getAssetTransfers
//!
//! Alchemy Documentation Reference:
//! - Token API: https://alchemy.com/docs/reference/token-api-overview.mdx
//! - Simulation: https://alchemy.com/docs/reference/simulation.mdx
//! - Prices API: https://alchemy.com/docs/reference/prices-api-quickstart.mdx
//! - Transfers API: https://alchemy.com/docs/reference/transfers-api-quickstart.mdx
//!
//! Compute Unit Costs (for rate limiting awareness):
//! - alchemy_getTokenMetadata: 10 CU
//! - alchemy_getTokenBalances: 20 CU
//! - alchemy_simulateAssetChanges: 2500 CU
//! - alchemy_getAssetTransfers: 120 CU
//! - Prices API: 40 CU per request

use eyre::{eyre, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use super::rpc::RpcProvider;

// ============================================
// TOKEN API TYPES
// ============================================

/// Token metadata from alchemy_getTokenMetadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenMetadata {
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub decimals: Option<u8>,
    pub logo: Option<String>,
}

/// Token balance entry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenBalance {
    pub contract_address: String,
    pub token_balance: Option<String>,
    pub error: Option<String>,
}

/// Response from alchemy_getTokenBalances
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenBalancesResponse {
    pub address: String,
    pub token_balances: Vec<TokenBalance>,
}

// ============================================
// TRANSACTION SIMULATION TYPES
// ============================================

/// Asset change from simulation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetChange {
    pub asset_type: String,
    pub change_type: String,
    pub from: String,
    pub to: String,
    pub raw_amount: Option<String>,
    pub amount: Option<String>,
    pub symbol: Option<String>,
    pub decimals: Option<u8>,
    pub contract_address: Option<String>,
    pub name: Option<String>,
    pub logo: Option<String>,
    pub token_id: Option<String>,
}

/// Simulation result from alchemy_simulateAssetChanges
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulationResponse {
    pub changes: Vec<AssetChange>,
    pub gas_used: Option<String>,
    pub error: Option<SimulationError>,
}

/// Simulation error details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationError {
    pub message: String,
    pub code: Option<i64>,
}

// ============================================
// PRICES API TYPES
// ============================================

/// Token price data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenPrice {
    pub network: String,
    pub address: String,
    pub prices: Vec<PriceEntry>,
}

/// Individual price entry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceEntry {
    pub currency: String,
    pub value: String,
    pub last_updated_at: String,
}

/// Prices API response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricesResponse {
    pub data: Vec<TokenPrice>,
}

// ============================================
// TRANSFERS API TYPES
// ============================================

/// Transfer category filter
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TransferCategory {
    External,
    Internal,
    Erc20,
    Erc721,
    Erc1155,
    Specialnft,
}

/// Asset transfer entry
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetTransfer {
    pub block_num: String,
    pub hash: String,
    pub from: String,
    pub to: Option<String>,
    pub value: Option<f64>,
    pub asset: Option<String>,
    pub category: String,
    pub raw_contract: Option<RawContract>,
}

/// Raw contract info in transfer
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawContract {
    pub value: Option<String>,
    pub address: Option<String>,
    pub decimal: Option<String>,
}

/// Response from alchemy_getAssetTransfers
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetTransfersResponse {
    pub transfers: Vec<AssetTransfer>,
    pub page_key: Option<String>,
}

// ============================================
// ALCHEMY API CLIENT
// ============================================

/// Alchemy Enhanced API Client
/// 
/// Provides access to Alchemy-specific APIs beyond standard JSON-RPC
pub struct AlchemyClient {
    provider: RpcProvider,
    #[allow(dead_code)]
    api_key: String,
}

impl AlchemyClient {
    /// Create new Alchemy client from existing RPC provider
    pub fn new(provider: RpcProvider) -> Result<Self> {
        let api_key = Self::get_api_key()?;
        Ok(Self { provider, api_key })
    }

    /// Get API key from environment
    fn get_api_key() -> Result<String> {
        if let Ok(key) = std::env::var("ALCHEMY_API_KEY") {
            if !key.is_empty() && key != "YOUR_API_KEY" {
                return Ok(key);
            }
        }
        Err(eyre!("ALCHEMY_API_KEY not configured"))
    }

    // ============================================
    // TOKEN API (10-20 CU)
    // ============================================

    /// Get token metadata (name, symbol, decimals, logo)
    /// 
    /// Compute Units: 10 CU
    /// Reference: https://alchemy.com/docs/reference/token-api-quickstart.mdx
    pub async fn get_token_metadata(&self, contract_address: &str) -> Result<TokenMetadata> {
        debug!("📊 Fetching token metadata for {}", contract_address);
        
        let params = serde_json::json!([contract_address]);
        self.provider.call::<TokenMetadata>("alchemy_getTokenMetadata", params).await
    }

    /// Get token balances for an address
    /// 
    /// Compute Units: 20 CU
    /// Reference: https://alchemy.com/docs/reference/token-api-quickstart.mdx
    pub async fn get_token_balances(
        &self,
        owner_address: &str,
        contract_addresses: Option<Vec<&str>>,
    ) -> Result<TokenBalancesResponse> {
        debug!("💰 Fetching token balances for {}", owner_address);
        
        let params = if let Some(contracts) = contract_addresses {
            serde_json::json!([owner_address, contracts])
        } else {
            // Use "DEFAULT_TOKENS" to get common tokens
            serde_json::json!([owner_address, "DEFAULT_TOKENS"])
        };
        
        self.provider.call::<TokenBalancesResponse>("alchemy_getTokenBalances", params).await
    }

    // ============================================
    // TRANSACTION SIMULATION (2500 CU)
    // ============================================

    /// Simulate transaction and get asset changes
    /// 
    /// Compute Units: 2500 CU (use sparingly!)
    /// Reference: https://alchemy.com/docs/reference/simulation-asset-changes.mdx
    /// 
    /// This is the MOST POWERFUL API for honeypot detection:
    /// - Shows exactly what tokens will be transferred
    /// - Detects hidden fees, taxes, and rug pulls
    /// - Preview transaction before sending
    pub async fn simulate_asset_changes(
        &self,
        from: &str,
        to: &str,
        value: Option<&str>,
        data: Option<&str>,
    ) -> Result<SimulationResponse> {
        info!("🔮 Simulating transaction: {} → {}", from, to);
        
        let mut tx = serde_json::json!({
            "from": from,
            "to": to,
        });
        
        if let Some(v) = value {
            tx["value"] = serde_json::Value::String(v.to_string());
        }
        if let Some(d) = data {
            tx["data"] = serde_json::Value::String(d.to_string());
        }
        
        let params = serde_json::json!([tx]);
        self.provider.call::<SimulationResponse>("alchemy_simulateAssetChanges", params).await
    }

    /// Simulate a swap transaction
    /// 
    /// Specialized helper for DEX swap simulation
    pub async fn simulate_swap(
        &self,
        from: &str,
        router_address: &str,
        swap_data: &str,
        value: Option<&str>,
    ) -> Result<SimulationResponse> {
        self.simulate_asset_changes(from, router_address, value, Some(swap_data)).await
    }

    // ============================================
    // TRANSFERS API (120 CU)
    // ============================================

    /// Get asset transfers for an address
    /// 
    /// Compute Units: 120 CU
    /// Reference: https://alchemy.com/docs/reference/transfers-api-quickstart.mdx
    #[allow(clippy::too_many_arguments)]
    pub async fn get_asset_transfers(
        &self,
        from_address: Option<&str>,
        to_address: Option<&str>,
        contract_addresses: Option<Vec<&str>>,
        categories: Vec<TransferCategory>,
        from_block: Option<&str>,
        to_block: Option<&str>,
        max_count: Option<u32>,
    ) -> Result<AssetTransfersResponse> {
        let mut params = serde_json::json!({
            "category": categories,
        });
        
        if let Some(from) = from_address {
            params["fromAddress"] = serde_json::Value::String(from.to_string());
        }
        if let Some(to) = to_address {
            params["toAddress"] = serde_json::Value::String(to.to_string());
        }
        if let Some(contracts) = contract_addresses {
            params["contractAddresses"] = serde_json::json!(contracts);
        }
        if let Some(from_blk) = from_block {
            params["fromBlock"] = serde_json::Value::String(from_blk.to_string());
        }
        if let Some(to_blk) = to_block {
            params["toBlock"] = serde_json::Value::String(to_blk.to_string());
        }
        if let Some(count) = max_count {
            params["maxCount"] = serde_json::Value::Number(count.into());
        }
        
        let request_params = serde_json::json!([params]);
        self.provider.call::<AssetTransfersResponse>("alchemy_getAssetTransfers", request_params).await
    }

    /// Get recent transfers for a token contract
    pub async fn get_token_transfers(
        &self,
        contract_address: &str,
        from_block: Option<&str>,
        max_count: Option<u32>,
    ) -> Result<AssetTransfersResponse> {
        self.get_asset_transfers(
            None,
            None,
            Some(vec![contract_address]),
            vec![TransferCategory::Erc20],
            from_block,
            Some("latest"),
            max_count,
        ).await
    }

    // ============================================
    // HELPER METHODS
    // ============================================

    /// Get underlying RPC provider
    pub fn provider(&self) -> &RpcProvider {
        &self.provider
    }

    /// Get chain ID
    pub fn chain_id(&self) -> u64 {
        self.provider.chain_id()
    }
}

// ============================================
// PRICES API CLIENT (Separate endpoint)
// ============================================

/// Alchemy Prices API Client
/// 
/// Uses separate REST endpoint: https://api.g.alchemy.com/prices/v1/{apiKey}/tokens/by-address
/// Compute Units: 40 CU per request
pub struct AlchemyPricesClient {
    client: reqwest::Client,
    api_key: String,
}

impl AlchemyPricesClient {
    /// Create new Prices API client
    pub fn new() -> Result<Self> {
        let api_key = std::env::var("ALCHEMY_API_KEY")
            .map_err(|_| eyre!("ALCHEMY_API_KEY not configured"))?;
        
        if api_key.is_empty() || api_key == "YOUR_API_KEY" {
            return Err(eyre!("Invalid ALCHEMY_API_KEY"));
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .gzip(true)
            .build()
            .map_err(|e| eyre!("Failed to build HTTP client: {}", e))?;

        Ok(Self { client, api_key })
    }

    /// Get token prices by address
    /// 
    /// Compute Units: 40 CU
    /// Reference: https://alchemy.com/docs/reference/prices-api-quickstart.mdx
    pub async fn get_token_prices(
        &self,
        network: &str,
        addresses: Vec<&str>,
    ) -> Result<PricesResponse> {
        let url = format!(
            "https://api.g.alchemy.com/prices/v1/{}/tokens/by-address",
            self.api_key
        );

        let body = serde_json::json!({
            "addresses": addresses.iter().map(|addr| {
                serde_json::json!({
                    "network": network,
                    "address": addr
                })
            }).collect::<Vec<_>>()
        });

        let response = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| eyre!("Prices API request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(eyre!("Prices API error: {}", response.status()));
        }

        response.json::<PricesResponse>().await
            .map_err(|e| eyre!("Failed to parse prices response: {}", e))
    }

    /// Get price for a single token
    pub async fn get_token_price(&self, network: &str, address: &str) -> Result<Option<f64>> {
        let response = self.get_token_prices(network, vec![address]).await?;
        
        if let Some(token) = response.data.first() {
            if let Some(price) = token.prices.iter().find(|p| p.currency == "usd") {
                return Ok(price.value.parse::<f64>().ok());
            }
        }
        
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transfer_category_serialization() {
        let cat = TransferCategory::Erc20;
        let json = serde_json::to_string(&cat).unwrap();
        assert_eq!(json, "\"erc20\"");
    }

    #[test]
    fn test_token_metadata_deserialization() {
        let json = r#"{
            "name": "Test Token",
            "symbol": "TEST",
            "decimals": 18,
            "logo": "https://example.com/logo.png"
        }"#;
        
        let metadata: TokenMetadata = serde_json::from_str(json).unwrap();
        assert_eq!(metadata.name, Some("Test Token".to_string()));
        assert_eq!(metadata.symbol, Some("TEST".to_string()));
        assert_eq!(metadata.decimals, Some(18));
    }
}
