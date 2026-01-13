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
    /// Labels (e.g., ["v3"] for Uniswap V3)
    #[serde(default)]
    pub labels: Vec<String>,
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

impl DexPair {
    /// Check if this pair is V2 compatible (not V3, not Velodrome/Aerodrome style)
    pub fn is_v2_compatible(&self) -> bool {
        // V3 pairs have "v3" or "v4" label
        let is_v3 = self.labels.iter().any(|l| l.contains("v3") || l.contains("v4"));
        
        // Velodrome/Aerodrome style DEXes are not V2 compatible
        let is_velodrome_style = matches!(
            self.dex_id.to_lowercase().as_str(),
            "velodrome" | "aerodrome" | "ramses" | "thena" | "equalizer"
        );
        
        !is_v3 && !is_velodrome_style
    }
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
    /// Prefers V2-compatible DEXes over V3
    pub async fn auto_detect_token(&self, token_address: &str) -> Result<AutoDetectedToken> {
        let pairs = self.get_token_pairs(token_address).await?;
        
        if pairs.is_empty() {
            return Err(eyre!("Token not found on any supported chain"));
        }

        // Separate V2 and V3 pairs
        let v2_pairs: Vec<&DexPair> = pairs.iter().filter(|p| p.is_v2_compatible()).collect();
        let all_pairs_count = pairs.len();
        let v2_pairs_count = v2_pairs.len();

        // Prefer V2 pairs, fallback to any pair for chain detection
        let best_pair = if !v2_pairs.is_empty() {
            v2_pairs[0]
        } else {
            // No V2 pairs, use first pair for chain detection but warn
            warn!("⚠️ No V2-compatible pairs found! Token may only be on V3/Velodrome-style DEXes");
            &pairs[0]
        };
        
        let chain_id = Self::dexscreener_name_to_chain_id(&best_pair.chain_id);
        let chain_name = Self::chain_id_to_name(chain_id);

        // Convert V2 pairs to DiscoveredDex (only V2 compatible ones)
        let all_discovered: Vec<DiscoveredDex> = pairs.iter()
            .filter(|p| Self::dexscreener_name_to_chain_id(&p.chain_id) == chain_id)
            .filter(|p| p.is_v2_compatible())
            .map(|p| p.to_discovered_dex())
            .collect();

        let best_dex = best_pair.to_discovered_dex();

        info!("🎯 Auto-detected: {} on {} (chain_id: {})", 
              best_pair.base_token.symbol.as_deref().unwrap_or("Unknown"),
              chain_name, chain_id);
        info!("   Total pairs: {}, V2-compatible: {}", all_pairs_count, v2_pairs_count);
        info!("   Best DEX: {} with ${:.2} liquidity (V2: {})", 
              best_dex.dex_name, best_dex.liquidity_usd, best_pair.is_v2_compatible());

        Ok(AutoDetectedToken {
            chain_id,
            chain_name: chain_name.to_string(),
            best_dex,
            token_name: best_pair.base_token.name.clone(),
            token_symbol: best_pair.base_token.symbol.clone(),
            all_pairs: all_discovered,
            // Add info about V3-only tokens
            has_v2_liquidity: v2_pairs_count > 0,
            total_pairs: all_pairs_count,
            // Market data from DexScreener
            price_usd: best_pair.price_usd.clone(),
            volume_24h_usd: best_pair.volume.as_ref().and_then(|v| v.h24),
            pair_address: Some(best_pair.pair_address.clone()),
        })
    }

    /// Convert numeric chain ID to DexScreener chain name (delegates to constants)
    fn chain_id_to_dexscreener_name(chain_id: u64) -> &'static str {
        crate::utils::constants::chain_id_to_dexscreener_name(chain_id)
    }

    /// Convert DexScreener chain name to numeric chain ID (delegates to constants)
    fn dexscreener_name_to_chain_id(name: &str) -> u64 {
        crate::utils::constants::dexscreener_name_to_chain_id(name)
    }

    /// Get human-readable chain name (delegates to constants)
    fn chain_id_to_name(chain_id: u64) -> &'static str {
        crate::utils::constants::get_chain_name(chain_id)
    }

    /// Public version of chain_id_to_name (delegates to constants)
    pub fn chain_id_to_name_pub(chain_id: u64) -> &'static str {
        crate::utils::constants::get_chain_name(chain_id)
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
    /// Best DEX to use (highest liquidity, prefers V2)
    pub best_dex: DiscoveredDex,
    /// Token name from DexScreener
    pub token_name: Option<String>,
    /// Token symbol from DexScreener
    pub token_symbol: Option<String>,
    /// All available V2-compatible pairs (sorted by liquidity)
    pub all_pairs: Vec<DiscoveredDex>,
    /// Whether token has V2-compatible liquidity
    pub has_v2_liquidity: bool,
    /// Total number of pairs (including V3)
    pub total_pairs: usize,
    // ============================================
    // Market Data from DexScreener (NEW!)
    // ============================================
    /// Price in USD
    pub price_usd: Option<String>,
    /// 24h trading volume
    pub volume_24h_usd: Option<f64>,
    /// Pair address
    pub pair_address: Option<String>,
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
    /// Only returns routers that are compatible with Uniswap V2 interface (getAmountsOut)
    fn dex_id_to_router(dex_id: &str, chain_id: &str) -> Option<String> {
        // Note: We only support Uniswap V2 style routers
        // V3, Aerodrome/Velodrome, and other AMMs have different interfaces
        match (dex_id.to_lowercase().as_str(), chain_id.to_lowercase().as_str()) {
            // Ethereum - V2 compatible
            ("uniswap", "ethereum") => Some("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".to_string()), // Uniswap V2
            ("sushiswap", "ethereum") => Some("0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F".to_string()),
            
            // BSC - V2 compatible
            ("pancakeswap", "bsc") => Some("0x10ED43C718714eb63d5aA57B78B54704E256024E".to_string()), // PancakeSwap V2
            ("biswap", "bsc") => Some("0x3a6d8cA21D1CF76F653A67577FA0D27453350dD8".to_string()),
            
            // Polygon - V2 compatible
            ("quickswap", "polygon") => Some("0xa5E0829CaCEd8fFDD4De3c43696c57F7D7A678ff".to_string()),
            ("sushiswap", "polygon") => Some("0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506".to_string()),
            
            // Arbitrum - V2 compatible
            ("camelot", "arbitrum") => Some("0xc873fEcbd354f5A56E00E710B90EF4201db2448d".to_string()),
            ("sushiswap", "arbitrum") => Some("0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506".to_string()),
            
            // Optimism - Velodrome is NOT V2 compatible, skip
            // ("velodrome", "optimism") => NOT SUPPORTED - different interface
            
            // Avalanche - V2 compatible
            ("traderjoe", "avalanche") => Some("0x60aE616a2155Ee3d9A68541Ba4544862310933d4".to_string()),
            ("pangolin", "avalanche") => Some("0xE54Ca86531e17Ef3616d22Ca28b0D458b6C89106".to_string()),
            
            // Base - Only V2 compatible routers
            // Aerodrome is NOT V2 compatible (Velodrome fork)
            // Uniswap on Base is V3 (NOT compatible)
            ("baseswap", "base") => Some("0x327Df1E6de05895d2ab08513aaDD9313Fe505d86".to_string()),
            ("sushiswap", "base") => Some("0x6BDED42c6DA8FBf0d2bA55B2fa120C5e0c8D7891".to_string()),
            // PancakeSwap V2 on Base
            ("pancakeswap", "base") => Some("0x02a84c1b3BBD7401a5f7fa98a384EBC70bB5749E".to_string()),
            
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
