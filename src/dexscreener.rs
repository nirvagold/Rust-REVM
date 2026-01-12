//! DexScreener API Client - "The Middleware Bridge"
//! 
//! ⚠️ IMPORTANT: DexScreener is used as "Route Discovery" ONLY!
//! 
//! ✅ USED FOR:
//! - Auto-detect chain (user doesn't need to specify chain_id)
//! - Find DEX with highest liquidity (avoid "no liquidity" errors)
//! - Token metadata (name, symbol) as fallback
//! - Price display (for UI cosmetics)
//!
//! ❌ NOT USED FOR:
//! - Security analysis (honeypot/tax detection)
//! - Real-time price for trading decisions
//!
//! Security analysis MUST use real-time REVM eth_call simulation!
//! DexScreener has 5-30 second delay - NOT suitable for security checks.
//!
//! API: https://api.dexscreener.com/latest/dex/tokens/{tokenAddress}
//! Free, no API key required

use eyre::{eyre, Result};
use serde::Deserialize;
use tracing::{info, warn};

/// DexScreener API response
#[derive(Debug, Deserialize)]
pub struct DexScreenerResponse {
    #[serde(default)]
    pub pairs: Option<Vec<DexPair>>,
}

/// A trading pair from DexScreener
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DexPair {
    /// Chain ID (e.g., "ethereum", "bsc", "polygon")
    pub chain_id: String,
    /// DEX identifier (e.g., "uniswap", "pancakeswap")
    pub dex_id: String,
    /// Pair address
    pub pair_address: String,
    /// Base token info
    pub base_token: DexToken,
    /// Quote token info (usually WETH/WBNB/USDT)
    pub quote_token: DexToken,
    /// Liquidity info
    pub liquidity: Option<DexLiquidity>,
    /// Price in USD
    pub price_usd: Option<String>,
    /// 24h volume
    pub volume: Option<DexVolume>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DexToken {
    pub address: String,
    pub name: Option<String>,
    pub symbol: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DexLiquidity {
    pub usd: Option<f64>,
    pub base: Option<f64>,
    pub quote: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DexVolume {
    pub h24: Option<f64>,
}

/// DexScreener API client
pub struct DexScreenerClient {
    client: reqwest::Client,
    base_url: String,
}

impl Default for DexScreenerClient {
    fn default() -> Self {
        Self::new()
    }
}

impl DexScreenerClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: "https://api.dexscreener.com/latest/dex".to_string(),
        }
    }

    /// Fetch all pairs for a token address
    /// Returns pairs sorted by liquidity (highest first)
    pub async fn get_token_pairs(&self, token_address: &str) -> Result<Vec<DexPair>> {
        let url = format!("{}/tokens/{}", self.base_url, token_address);
        
        info!("🔍 DexScreener: Fetching pairs for {}", token_address);

        let response = self.client
            .get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| eyre!("DexScreener request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(eyre!("DexScreener API error: {}", response.status()));
        }

        let data: DexScreenerResponse = response.json().await
            .map_err(|e| eyre!("Failed to parse DexScreener response: {}", e))?;

        let mut pairs = data.pairs.unwrap_or_default();
        
        // Sort by liquidity (highest first)
        pairs.sort_by(|a, b| {
            let liq_a = a.liquidity.as_ref().and_then(|l| l.usd).unwrap_or(0.0);
            let liq_b = b.liquidity.as_ref().and_then(|l| l.usd).unwrap_or(0.0);
            liq_b.partial_cmp(&liq_a).unwrap_or(std::cmp::Ordering::Equal)
        });

        info!("📊 DexScreener: Found {} pairs", pairs.len());
        
        Ok(pairs)
    }

    /// Get pairs for a specific chain
    pub async fn get_pairs_for_chain(&self, token_address: &str, chain_id: u64) -> Result<Vec<DexPair>> {
        let pairs = self.get_token_pairs(token_address).await?;
        
        let chain_name = Self::chain_id_to_dexscreener_name(chain_id);
        
        let filtered: Vec<DexPair> = pairs
            .into_iter()
            .filter(|p| p.chain_id.to_lowercase() == chain_name.to_lowercase())
            .collect();

        info!("📊 DexScreener: {} pairs on chain {} ({})", 
              filtered.len(), chain_name, chain_id);

        Ok(filtered)
    }

    /// Get the best pair (highest liquidity) for a chain
    pub async fn get_best_pair(&self, token_address: &str, chain_id: u64) -> Option<DexPair> {
        match self.get_pairs_for_chain(token_address, chain_id).await {
            Ok(pairs) => pairs.into_iter().next(),
            Err(e) => {
                warn!("⚠️ DexScreener error: {}", e);
                None
            }
        }
    }

    /// 🎯 AUTO-DETECT: Find which chain a token is on and best DEX to use
    /// This is the "Unified Entry Point" - user only needs to provide address
    pub async fn auto_detect_token(&self, token_address: &str) -> Result<AutoDetectedToken> {
        let pairs = self.get_token_pairs(token_address).await?;
        
        if pairs.is_empty() {
            return Err(eyre!("Token not found on any supported chain"));
        }

        // Get the pair with highest liquidity
        let best_pair = pairs.first().ok_or_else(|| eyre!("No pairs found"))?;
        
        let chain_id = Self::dexscreener_name_to_chain_id(&best_pair.chain_id);
        let chain_name = Self::chain_id_to_name(chain_id);

        // Convert all pairs to DiscoveredDex
        let all_pairs: Vec<DiscoveredDex> = pairs.iter()
            .filter(|p| Self::dexscreener_name_to_chain_id(&p.chain_id) == chain_id)
            .map(|p| p.to_discovered_dex())
            .collect();

        let best_dex = best_pair.to_discovered_dex();

        info!("🎯 Auto-detected: {} on {} (chain_id: {})", 
              best_pair.base_token.symbol.as_deref().unwrap_or("Unknown"),
              chain_name, chain_id);
        info!("   Best DEX: {} with ${:.2} liquidity", 
              best_dex.dex_name, best_dex.liquidity_usd);

        Ok(AutoDetectedToken {
            chain_id,
            chain_name: chain_name.to_string(),
            best_dex,
            token_name: best_pair.base_token.name.clone(),
            token_symbol: best_pair.base_token.symbol.clone(),
            all_pairs,
        })
    }

    /// Convert numeric chain ID to DexScreener chain name
    fn chain_id_to_dexscreener_name(chain_id: u64) -> &'static str {
        match chain_id {
            1 => "ethereum",
            56 => "bsc",
            137 => "polygon",
            42161 => "arbitrum",
            10 => "optimism",
            43114 => "avalanche",
            8453 => "base",
            _ => "ethereum",
        }
    }

    /// Convert DexScreener chain name to numeric chain ID
    fn dexscreener_name_to_chain_id(name: &str) -> u64 {
        match name.to_lowercase().as_str() {
            "ethereum" => 1,
            "bsc" => 56,
            "polygon" => 137,
            "arbitrum" => 42161,
            "optimism" => 10,
            "avalanche" => 43114,
            "base" => 8453,
            "solana" => 0, // Not supported
            _ => 1, // Default to Ethereum
        }
    }

    /// Get human-readable chain name
    fn chain_id_to_name(chain_id: u64) -> &'static str {
        match chain_id {
            1 => "Ethereum",
            56 => "BNB Smart Chain",
            137 => "Polygon",
            42161 => "Arbitrum One",
            10 => "Optimism",
            43114 => "Avalanche C-Chain",
            8453 => "Base",
            _ => "Unknown",
        }
    }
}

/// Discovered DEX info from DexScreener
#[derive(Debug, Clone)]
pub struct DiscoveredDex {
    pub dex_id: String,
    pub dex_name: String,
    pub pair_address: String,
    pub router_address: Option<String>,
    pub liquidity_usd: f64,
    pub quote_token: String,
}

/// Auto-detected chain and DEX info for a token
/// Used when user doesn't specify chain_id
#[derive(Debug, Clone)]
pub struct AutoDetectedToken {
    /// Numeric chain ID (1, 56, 137, etc.)
    pub chain_id: u64,
    /// Chain name (Ethereum, BSC, etc.)
    pub chain_name: String,
    /// Best DEX to use (highest liquidity)
    pub best_dex: DiscoveredDex,
    /// Token name from DexScreener
    pub token_name: Option<String>,
    /// Token symbol from DexScreener
    pub token_symbol: Option<String>,
    /// All available pairs (sorted by liquidity)
    pub all_pairs: Vec<DiscoveredDex>,
}

impl DexPair {
    /// Convert to DiscoveredDex with router address lookup
    pub fn to_discovered_dex(&self) -> DiscoveredDex {
        let router = Self::dex_id_to_router(&self.dex_id, &self.chain_id);
        let liquidity = self.liquidity.as_ref().and_then(|l| l.usd).unwrap_or(0.0);
        
        DiscoveredDex {
            dex_id: self.dex_id.clone(),
            dex_name: Self::dex_id_to_name(&self.dex_id),
            pair_address: self.pair_address.clone(),
            router_address: router,
            liquidity_usd: liquidity,
            quote_token: self.quote_token.symbol.clone().unwrap_or_default(),
        }
    }

    /// Map DexScreener dex_id to router address
    fn dex_id_to_router(dex_id: &str, chain_id: &str) -> Option<String> {
        match (dex_id.to_lowercase().as_str(), chain_id.to_lowercase().as_str()) {
            // Ethereum
            ("uniswap", "ethereum") => Some("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".to_string()),
            ("sushiswap", "ethereum") => Some("0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F".to_string()),
            
            // BSC
            ("pancakeswap", "bsc") => Some("0x10ED43C718714eb63d5aA57B78B54704E256024E".to_string()),
            ("biswap", "bsc") => Some("0x3a6d8cA21D1CF76F653A67577FA0D27453350dD8".to_string()),
            
            // Polygon
            ("quickswap", "polygon") => Some("0xa5E0829CaCEd8fFDD4De3c43696c57F7D7A678ff".to_string()),
            ("sushiswap", "polygon") => Some("0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506".to_string()),
            
            // Arbitrum
            ("camelot", "arbitrum") => Some("0xc873fEcbd354f5A56E00E710B90EF4201db2448d".to_string()),
            ("sushiswap", "arbitrum") => Some("0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506".to_string()),
            
            // Optimism
            ("velodrome", "optimism") => Some("0xa062aE8A9c5e11aaA026fc2670B0D65cCc8B2858".to_string()),
            
            // Avalanche
            ("traderjoe", "avalanche") => Some("0x60aE616a2155Ee3d9A68541Ba4544862310933d4".to_string()),
            ("pangolin", "avalanche") => Some("0xE54Ca86531e17Ef3616d22Ca28b0D458b6C89106".to_string()),
            
            // Base
            ("aerodrome", "base") => Some("0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43".to_string()),
            ("baseswap", "base") => Some("0x327Df1E6de05895d2ab08513aaDD9313Fe505d86".to_string()),
            ("sushiswap", "base") => Some("0x6BDED42c6DA8FBf0d2bA55B2fa120C5e0c8D7891".to_string()),
            ("uniswap", "base") => Some("0x2626664c2603336E57B271c5C0b26F421741e481".to_string()),
            
            _ => None,
        }
    }

    /// Map dex_id to human-readable name
    fn dex_id_to_name(dex_id: &str) -> String {
        match dex_id.to_lowercase().as_str() {
            "uniswap" => "Uniswap V2".to_string(),
            "sushiswap" => "SushiSwap".to_string(),
            "pancakeswap" => "PancakeSwap V2".to_string(),
            "biswap" => "BiSwap".to_string(),
            "quickswap" => "QuickSwap".to_string(),
            "camelot" => "Camelot".to_string(),
            "velodrome" => "Velodrome".to_string(),
            "traderjoe" => "TraderJoe".to_string(),
            "pangolin" => "Pangolin".to_string(),
            "aerodrome" => "Aerodrome".to_string(),
            "baseswap" => "BaseSwap".to_string(),
            _ => dex_id.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dexscreener_client() {
        let client = DexScreenerClient::new();
        // Test with USDT on Ethereum
        let pairs = client.get_token_pairs("0xdAC17F958D2ee523a2206206994597C13D831ec7").await;
        assert!(pairs.is_ok());
    }
}
