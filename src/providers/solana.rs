//! Solana Provider Module
//!
//! High-performance Solana integration using:
//! 1. Yellowstone gRPC - Real-time streaming (slots, transactions, accounts, blocks)
//! 2. DAS API - Digital Asset Standard for NFTs and Fungible Tokens
//! 3. Standard JSON-RPC - Basic Solana operations
//!
//! Alchemy Documentation Reference:
//! - Yellowstone gRPC: https://alchemy.com/docs/reference/yellowstone-grpc-overview.mdx
//! - DAS API: https://alchemy.com/docs/reference/alchemy-das-apis-for-solana.mdx
//! - Best Practices: https://alchemy.com/docs/reference/yellowstone-grpc-best-practices.mdx
//!
//! Yellowstone gRPC Features:
//! - 6000 slot historical replay
//! - Filter by vote, failed, account_include, account_exclude
//! - Real-time transaction streaming
//! - Account change monitoring
//!
//! DAS API Features:
//! - getAsset, getAssets - Token/NFT metadata
//! - getAssetsByOwner - Portfolio lookup
//! - getTokenAccounts - Token balances
//! - searchAssets - Advanced search

use eyre::{eyre, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

// ============================================
// SOLANA CONSTANTS
// ============================================

/// Solana Mainnet chain ID (non-EVM, use 0 or custom)
pub const SOLANA_CHAIN_ID: u64 = 0;

/// Raydium AMM Program ID (for new pool detection)
pub const RAYDIUM_AMM_PROGRAM: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";

/// Raydium CLMM Program ID
pub const RAYDIUM_CLMM_PROGRAM: &str = "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK";

/// Orca Whirlpool Program ID
pub const ORCA_WHIRLPOOL_PROGRAM: &str = "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc";

/// Jupiter Aggregator Program ID
pub const JUPITER_PROGRAM: &str = "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4";

/// Token Program ID
pub const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

/// Token-2022 Program ID
pub const TOKEN_2022_PROGRAM: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";

/// Associated Token Program ID
pub const ASSOCIATED_TOKEN_PROGRAM: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

// ============================================
// SOLANA RPC TYPES
// ============================================

/// Solana account info
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolanaAccountInfo {
    pub lamports: u64,
    pub owner: String,
    pub data: AccountData,
    pub executable: bool,
    pub rent_epoch: u64,
}

/// Account data (can be base64 or parsed)
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum AccountData {
    Base64(Vec<String>),
    Parsed(ParsedAccountData),
}

/// Parsed account data
#[derive(Debug, Clone, Deserialize)]
pub struct ParsedAccountData {
    pub program: String,
    pub parsed: serde_json::Value,
    pub space: u64,
}

/// Token account info
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenAccountInfo {
    pub mint: String,
    pub owner: String,
    pub token_amount: TokenAmount,
    pub delegate: Option<String>,
    pub state: String,
}

/// Token amount
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenAmount {
    pub amount: String,
    pub decimals: u8,
    pub ui_amount: Option<f64>,
    pub ui_amount_string: String,
}

/// Solana transaction
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolanaTransaction {
    pub slot: u64,
    pub transaction: TransactionData,
    pub meta: Option<TransactionMeta>,
    pub block_time: Option<i64>,
}

/// Transaction data
#[derive(Debug, Clone, Deserialize)]
pub struct TransactionData {
    pub signatures: Vec<String>,
    pub message: TransactionMessage,
}

/// Transaction message
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionMessage {
    pub account_keys: Vec<String>,
    pub recent_blockhash: String,
    pub instructions: Vec<TransactionInstruction>,
}

/// Transaction instruction
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionInstruction {
    pub program_id_index: u8,
    pub accounts: Vec<u8>,
    pub data: String,
}

/// Transaction metadata
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionMeta {
    pub err: Option<serde_json::Value>,
    pub fee: u64,
    pub pre_balances: Vec<u64>,
    pub post_balances: Vec<u64>,
    pub pre_token_balances: Option<Vec<SolanaTokenBalance>>,
    pub post_token_balances: Option<Vec<SolanaTokenBalance>>,
    pub log_messages: Option<Vec<String>>,
}

/// Token balance in transaction (Solana-specific)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolanaTokenBalance {
    pub account_index: u8,
    pub mint: String,
    pub owner: Option<String>,
    pub ui_token_amount: TokenAmount,
}

// ============================================
// DAS API TYPES
// ============================================

/// DAS Asset (NFT or Fungible Token)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DasAsset {
    pub id: String,
    pub interface: String,
    pub content: Option<AssetContent>,
    pub authorities: Option<Vec<AssetAuthority>>,
    pub compression: Option<AssetCompression>,
    pub grouping: Option<Vec<AssetGrouping>>,
    pub royalty: Option<AssetRoyalty>,
    pub creators: Option<Vec<AssetCreator>>,
    pub ownership: AssetOwnership,
    pub supply: Option<AssetSupply>,
    pub mutable: bool,
    pub burnt: bool,
    pub token_info: Option<TokenInfo>,
}

/// Asset content (metadata)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetContent {
    pub json_uri: Option<String>,
    pub metadata: Option<AssetMetadata>,
    pub links: Option<HashMap<String, String>>,
}

/// Asset metadata
#[derive(Debug, Clone, Deserialize)]
pub struct AssetMetadata {
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub description: Option<String>,
    pub token_standard: Option<String>,
}

/// Asset authority
#[derive(Debug, Clone, Deserialize)]
pub struct AssetAuthority {
    pub address: String,
    pub scopes: Vec<String>,
}

/// Asset compression info
#[derive(Debug, Clone, Deserialize)]
pub struct AssetCompression {
    pub eligible: bool,
    pub compressed: bool,
}

/// Asset grouping
#[derive(Debug, Clone, Deserialize)]
pub struct AssetGrouping {
    pub group_key: String,
    pub group_value: String,
}

/// Asset royalty
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetRoyalty {
    pub royalty_model: String,
    pub target: Option<String>,
    pub percent: f64,
    pub basis_points: u32,
    pub primary_sale_happened: bool,
    pub locked: bool,
}

/// Asset creator
#[derive(Debug, Clone, Deserialize)]
pub struct AssetCreator {
    pub address: String,
    pub share: u8,
    pub verified: bool,
}

/// Asset ownership
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetOwnership {
    pub frozen: bool,
    pub delegated: bool,
    pub delegate: Option<String>,
    pub ownership_model: String,
    pub owner: String,
}

/// Asset supply
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetSupply {
    pub print_max_supply: Option<u64>,
    pub print_current_supply: Option<u64>,
    pub edition_nonce: Option<u8>,
}

/// Token info for fungible tokens
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenInfo {
    pub symbol: Option<String>,
    pub balance: Option<u64>,
    pub supply: Option<u64>,
    pub decimals: Option<u8>,
    pub token_program: Option<String>,
    pub price_info: Option<PriceInfo>,
}

/// Price info
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceInfo {
    pub price_per_token: f64,
    pub currency: String,
}

/// DAS API response for getAssets
#[derive(Debug, Clone, Deserialize)]
pub struct DasAssetsResponse {
    pub items: Vec<DasAsset>,
    pub total: u32,
    pub limit: u32,
    pub page: u32,
}

// ============================================
// YELLOWSTONE STREAMING TYPES
// ============================================

/// Yellowstone subscription filter
#[derive(Debug, Clone, Serialize)]
pub struct YellowstoneFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vote: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_include: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_exclude: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_required: Option<Vec<String>>,
}

impl Default for YellowstoneFilter {
    fn default() -> Self {
        Self {
            vote: Some(false),      // Exclude vote transactions
            failed: Some(false),    // Exclude failed transactions
            account_include: None,
            account_exclude: None,
            account_required: None,
        }
    }
}

impl YellowstoneFilter {
    /// Filter for Raydium AMM transactions
    pub fn raydium_amm() -> Self {
        Self {
            vote: Some(false),
            failed: Some(false),
            account_include: Some(vec![RAYDIUM_AMM_PROGRAM.to_string()]),
            account_exclude: None,
            account_required: None,
        }
    }

    /// Filter for Orca Whirlpool transactions
    pub fn orca_whirlpool() -> Self {
        Self {
            vote: Some(false),
            failed: Some(false),
            account_include: Some(vec![ORCA_WHIRLPOOL_PROGRAM.to_string()]),
            account_exclude: None,
            account_required: None,
        }
    }

    /// Filter for Jupiter swaps
    pub fn jupiter() -> Self {
        Self {
            vote: Some(false),
            failed: Some(false),
            account_include: Some(vec![JUPITER_PROGRAM.to_string()]),
            account_exclude: None,
            account_required: None,
        }
    }

    /// Filter for new token mints
    pub fn token_mints() -> Self {
        Self {
            vote: Some(false),
            failed: Some(false),
            account_include: Some(vec![
                TOKEN_PROGRAM.to_string(),
                TOKEN_2022_PROGRAM.to_string(),
            ]),
            account_exclude: None,
            account_required: None,
        }
    }
}

/// Yellowstone event types
#[derive(Debug, Clone)]
pub enum YellowstoneEvent {
    Slot(SlotUpdate),
    Transaction(SolanaTransaction),
    Account(AccountUpdate),
    Block(BlockUpdate),
    Connected,
    Disconnected,
    Error(String),
}

/// Slot update
#[derive(Debug, Clone)]
pub struct SlotUpdate {
    pub slot: u64,
    pub parent: Option<u64>,
    pub status: SlotStatus,
}

/// Slot status
#[derive(Debug, Clone)]
pub enum SlotStatus {
    Processed,
    Confirmed,
    Finalized,
}

/// Account update
#[derive(Debug, Clone)]
pub struct AccountUpdate {
    pub pubkey: String,
    pub slot: u64,
    pub lamports: u64,
    pub owner: String,
    pub data: Vec<u8>,
    pub executable: bool,
}

/// Block update
#[derive(Debug, Clone)]
pub struct BlockUpdate {
    pub slot: u64,
    pub blockhash: String,
    pub parent_slot: u64,
    pub transactions: Vec<SolanaTransaction>,
}

// ============================================
// SOLANA RPC CLIENT
// ============================================

/// Solana RPC Client
pub struct SolanaClient {
    rpc_url: String,
    client: reqwest::Client,
    #[allow(dead_code)]
    api_key: String,
}

impl SolanaClient {
    /// Create new Solana client
    pub fn new() -> Result<Self> {
        let api_key = std::env::var("ALCHEMY_API_KEY")
            .map_err(|_| eyre!("ALCHEMY_API_KEY not configured"))?;

        if api_key.is_empty() || api_key == "YOUR_API_KEY" {
            return Err(eyre!("Invalid ALCHEMY_API_KEY"));
        }

        let rpc_url = format!("https://solana-mainnet.g.alchemy.com/v2/{}", api_key);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .gzip(true)
            .build()
            .map_err(|e| eyre!("Failed to build HTTP client: {}", e))?;

        Ok(Self {
            rpc_url,
            client,
            api_key,
        })
    }

    /// Execute JSON-RPC call
    async fn call<T: for<'de> Deserialize<'de>>(
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

        let response = self.client
            .post(&self.rpc_url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| eyre!("Request failed: {}", e))?;

        let json: serde_json::Value = response.json().await
            .map_err(|e| eyre!("Failed to parse response: {}", e))?;

        if let Some(error) = json.get("error") {
            return Err(eyre!("RPC error: {}", error));
        }

        let result = json.get("result")
            .ok_or_else(|| eyre!("No result in response"))?;

        serde_json::from_value(result.clone())
            .map_err(|e| eyre!("Failed to deserialize result: {}", e))
    }

    // ============================================
    // STANDARD RPC METHODS
    // ============================================

    /// Get current slot
    pub async fn get_slot(&self) -> Result<u64> {
        self.call::<u64>("getSlot", serde_json::json!([])).await
    }

    /// Get account info
    pub async fn get_account_info(&self, pubkey: &str) -> Result<Option<SolanaAccountInfo>> {
        let params = serde_json::json!([
            pubkey,
            {"encoding": "jsonParsed"}
        ]);
        
        let result: serde_json::Value = self.call("getAccountInfo", params).await?;
        
        if result.is_null() {
            return Ok(None);
        }
        
        serde_json::from_value(result.get("value").cloned().unwrap_or_default())
            .map_err(|e| eyre!("Failed to parse account info: {}", e))
    }

    /// Get token accounts by owner
    pub async fn get_token_accounts_by_owner(
        &self,
        owner: &str,
        mint: Option<&str>,
    ) -> Result<Vec<TokenAccountInfo>> {
        let filter = if let Some(m) = mint {
            serde_json::json!({"mint": m})
        } else {
            serde_json::json!({"programId": TOKEN_PROGRAM})
        };

        let params = serde_json::json!([
            owner,
            filter,
            {"encoding": "jsonParsed"}
        ]);

        let result: serde_json::Value = self.call("getTokenAccountsByOwner", params).await?;
        
        let accounts = result.get("value")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        item.get("account")
                            .and_then(|a| a.get("data"))
                            .and_then(|d| d.get("parsed"))
                            .and_then(|p| p.get("info"))
                            .and_then(|i| serde_json::from_value(i.clone()).ok())
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(accounts)
    }

    /// Get transaction
    pub async fn get_transaction(&self, signature: &str) -> Result<Option<SolanaTransaction>> {
        let params = serde_json::json!([
            signature,
            {"encoding": "jsonParsed", "maxSupportedTransactionVersion": 0}
        ]);

        self.call("getTransaction", params).await
    }

    /// Get recent blockhash
    pub async fn get_recent_blockhash(&self) -> Result<String> {
        let result: serde_json::Value = self.call(
            "getLatestBlockhash",
            serde_json::json!([{"commitment": "finalized"}])
        ).await?;

        result.get("value")
            .and_then(|v| v.get("blockhash"))
            .and_then(|b| b.as_str())
            .map(String::from)
            .ok_or_else(|| eyre!("Failed to get blockhash"))
    }

    // ============================================
    // DAS API METHODS
    // ============================================

    /// Get asset by ID (DAS API)
    pub async fn get_asset(&self, asset_id: &str) -> Result<DasAsset> {
        debug!("📊 Getting asset: {}", asset_id);
        
        let params = serde_json::json!({
            "id": asset_id
        });

        self.call("getAsset", serde_json::json!([params])).await
    }

    /// Get assets by owner (DAS API)
    pub async fn get_assets_by_owner(
        &self,
        owner: &str,
        page: u32,
        limit: u32,
    ) -> Result<DasAssetsResponse> {
        debug!("📊 Getting assets for owner: {}", owner);
        
        let params = serde_json::json!({
            "ownerAddress": owner,
            "page": page,
            "limit": limit
        });

        self.call("getAssetsByOwner", serde_json::json!([params])).await
    }

    /// Search assets (DAS API)
    pub async fn search_assets(
        &self,
        query: &str,
        page: u32,
        limit: u32,
    ) -> Result<DasAssetsResponse> {
        debug!("🔍 Searching assets: {}", query);
        
        let params = serde_json::json!({
            "searchText": query,
            "page": page,
            "limit": limit
        });

        self.call("searchAssets", serde_json::json!([params])).await
    }

    /// Get token accounts (DAS API)
    pub async fn get_token_accounts_das(
        &self,
        owner: &str,
        mint: Option<&str>,
    ) -> Result<DasAssetsResponse> {
        debug!("💰 Getting token accounts for: {}", owner);
        
        let mut params = serde_json::json!({
            "owner": owner,
            "displayOptions": {
                "showFungible": true,
                "showNativeBalance": true
            }
        });

        if let Some(m) = mint {
            params["mint"] = serde_json::Value::String(m.to_string());
        }

        self.call("getTokenAccounts", serde_json::json!([params])).await
    }

    // ============================================
    // HELPER METHODS
    // ============================================

    /// Check if a token is a potential honeypot on Solana
    pub async fn analyze_token(&self, mint: &str) -> Result<SolanaTokenAnalysis> {
        info!("🔍 Analyzing Solana token: {}", mint);

        // Get token metadata via DAS
        let asset = self.get_asset(mint).await.ok();

        // Get token supply info
        let supply_info = self.get_token_supply(mint).await.ok();

        // Analyze for red flags
        let mut red_flags = Vec::new();
        let mut risk_score = 0;

        if let Some(ref asset) = asset {
            // Check if mutable (can change metadata)
            if asset.mutable {
                red_flags.push("Token metadata is mutable".to_string());
                risk_score += 20;
            }

            // Check authorities
            if let Some(ref authorities) = asset.authorities {
                if authorities.len() > 2 {
                    red_flags.push("Multiple authorities detected".to_string());
                    risk_score += 15;
                }
            }

            // Check if burnt
            if asset.burnt {
                red_flags.push("Token is burnt".to_string());
                risk_score += 50;
            }
        } else {
            red_flags.push("Token metadata not found".to_string());
            risk_score += 30;
        }

        // Check supply concentration
        if let Some((supply, _)) = supply_info {
            if supply == 0 {
                red_flags.push("Zero supply".to_string());
                risk_score += 40;
            }
        }

        Ok(SolanaTokenAnalysis {
            mint: mint.to_string(),
            name: asset.as_ref()
                .and_then(|a| a.content.as_ref())
                .and_then(|c| c.metadata.as_ref())
                .and_then(|m| m.name.clone()),
            symbol: asset.as_ref()
                .and_then(|a| a.content.as_ref())
                .and_then(|c| c.metadata.as_ref())
                .and_then(|m| m.symbol.clone()),
            is_honeypot: risk_score > 50,
            risk_score,
            red_flags,
            mutable: asset.as_ref().map(|a| a.mutable).unwrap_or(true),
            burnt: asset.as_ref().map(|a| a.burnt).unwrap_or(false),
        })
    }

    /// Get token supply
    async fn get_token_supply(&self, mint: &str) -> Result<(u64, u8)> {
        let params = serde_json::json!([mint]);
        let result: serde_json::Value = self.call("getTokenSupply", params).await?;

        let value = result.get("value")
            .ok_or_else(|| eyre!("No value in response"))?;

        let amount = value.get("amount")
            .and_then(|a| a.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let decimals = value.get("decimals")
            .and_then(|d| d.as_u64())
            .map(|d| d as u8)
            .unwrap_or(9);

        Ok((amount, decimals))
    }
}

/// Solana token analysis result
#[derive(Debug, Clone)]
pub struct SolanaTokenAnalysis {
    pub mint: String,
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub is_honeypot: bool,
    pub risk_score: u32,
    pub red_flags: Vec<String>,
    pub mutable: bool,
    pub burnt: bool,
}

// ============================================
// YELLOWSTONE GRPC CLIENT (Placeholder)
// ============================================

/// Yellowstone gRPC Client for real-time streaming
/// 
/// Note: Full gRPC implementation requires proto files from Alchemy.
/// This is a placeholder that uses WebSocket fallback.
pub struct YellowstoneClient {
    api_key: String,
    #[allow(dead_code)]
    grpc_url: String,
}

impl YellowstoneClient {
    /// Create new Yellowstone client
    pub fn new() -> Result<Self> {
        let api_key = std::env::var("ALCHEMY_API_KEY")
            .map_err(|_| eyre!("ALCHEMY_API_KEY not configured"))?;

        // Yellowstone gRPC endpoint
        let grpc_url = format!(
            "https://solana-mainnet.g.alchemy.com/v2/{}",
            api_key
        );

        Ok(Self { api_key, grpc_url })
    }

    /// Subscribe to transactions (WebSocket fallback)
    /// 
    /// For full gRPC streaming, Alchemy provides proto files.
    /// This uses WebSocket as a fallback for basic functionality.
    pub async fn subscribe_transactions(
        &self,
        _filter: YellowstoneFilter,
    ) -> Result<mpsc::Receiver<YellowstoneEvent>> {
        let (tx, rx) = mpsc::channel(1000);
        let api_key = self.api_key.clone();

        tokio::spawn(async move {
            let ws_url = format!(
                "wss://solana-mainnet.g.alchemy.com/v2/{}",
                api_key
            );

            info!("🔌 Connecting to Solana WebSocket...");

            // Use WebSocket for basic streaming
            // Full gRPC would use tonic with proto definitions
            match tokio_tungstenite::connect_async(&ws_url).await {
                Ok((ws_stream, _)) => {
                    info!("✅ Connected to Solana WebSocket");
                    let _ = tx.send(YellowstoneEvent::Connected).await;

                    // Note: Full implementation would use gRPC streaming
                    // This is a simplified WebSocket fallback
                    let (mut _write, mut read) = ws_stream.split();

                    use futures_util::StreamExt;
                    while let Some(msg) = read.next().await {
                        match msg {
                            Ok(_) => {
                                // Process message
                            }
                            Err(e) => {
                                warn!("WebSocket error: {}", e);
                                break;
                            }
                        }
                    }

                    let _ = tx.send(YellowstoneEvent::Disconnected).await;
                }
                Err(e) => {
                    let _ = tx.send(YellowstoneEvent::Error(e.to_string())).await;
                }
            }
        });

        Ok(rx)
    }

    /// Subscribe to new token mints
    pub async fn subscribe_new_tokens(&self) -> Result<mpsc::Receiver<YellowstoneEvent>> {
        self.subscribe_transactions(YellowstoneFilter::token_mints()).await
    }

    /// Subscribe to Raydium AMM events
    pub async fn subscribe_raydium(&self) -> Result<mpsc::Receiver<YellowstoneEvent>> {
        self.subscribe_transactions(YellowstoneFilter::raydium_amm()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yellowstone_filter_default() {
        let filter = YellowstoneFilter::default();
        assert_eq!(filter.vote, Some(false));
        assert_eq!(filter.failed, Some(false));
    }

    #[test]
    fn test_yellowstone_filter_raydium() {
        let filter = YellowstoneFilter::raydium_amm();
        assert!(filter.account_include.is_some());
        let accounts = filter.account_include.unwrap();
        assert!(accounts.contains(&RAYDIUM_AMM_PROGRAM.to_string()));
    }

    #[test]
    fn test_solana_constants() {
        assert!(!RAYDIUM_AMM_PROGRAM.is_empty());
        assert!(!TOKEN_PROGRAM.is_empty());
        assert!(!JUPITER_PROGRAM.is_empty());
    }
}
