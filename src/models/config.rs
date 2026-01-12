//! Configuration module for Mempool Sentry
//!
//! CEO Directive: Uses constants from utils/constants.rs
//! No hardcoded addresses or chain IDs in this file!

use alloy_primitives::Address;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::time::Duration;
use tracing::info;

use crate::utils::constants::{
    build_alchemy_url, get_chain_name, get_dex_routers, get_native_symbol,
    get_public_rpc_fallback, get_weth_address, CHAIN_ID_ARBITRUM, CHAIN_ID_AVALANCHE,
    CHAIN_ID_BASE, CHAIN_ID_BSC, CHAIN_ID_ETHEREUM, CHAIN_ID_OPTIMISM, CHAIN_ID_POLYGON,
    SUPPORTED_CHAIN_IDS,
};

/// Supported blockchain networks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChainId {
    Ethereum = 1,
    BinanceSmartChain = 56,
    Polygon = 137,
    Arbitrum = 42161,
    Optimism = 10,
    Avalanche = 43114,
    Base = 8453,
}

impl ChainId {
    /// Get chain from numeric ID
    pub fn from_id(id: u64) -> Option<Self> {
        match id {
            1 => Some(Self::Ethereum),
            56 => Some(Self::BinanceSmartChain),
            137 => Some(Self::Polygon),
            42161 => Some(Self::Arbitrum),
            10 => Some(Self::Optimism),
            43114 => Some(Self::Avalanche),
            8453 => Some(Self::Base),
            _ => None,
        }
    }

    /// Get chain name (delegates to constants)
    pub fn name(&self) -> &'static str {
        get_chain_name(*self as u64)
    }

    /// Get chain symbol (delegates to constants)
    pub fn symbol(&self) -> &'static str {
        get_native_symbol(*self as u64)
    }
}

/// DEX Router info
#[derive(Debug, Clone)]
pub struct DexRouter {
    pub name: String,
    pub address: Address,
}

/// Chain-specific configuration (WETH, Routers, RPC)
#[derive(Debug, Clone)]
pub struct ChainConfig {
    pub chain_id: ChainId,
    pub name: String,
    pub symbol: String,
    pub weth: Address,
    pub routers: Vec<DexRouter>,
    pub rpc_url: String,
}

impl ChainConfig {
    /// Get Alchemy API key from environment
    /// CEO Security Directive: Key is NEVER logged
    fn get_alchemy_key() -> Option<String> {
        // First try dedicated ALCHEMY_API_KEY env var
        if let Ok(key) = std::env::var("ALCHEMY_API_KEY") {
            if !key.is_empty() && key != "YOUR_API_KEY" {
                info!("🔑 ALCHEMY_API_KEY configured (key hidden for security)");
                return Some(key);
            }
        }

        // Fallback: extract from ETH_HTTP_URL
        std::env::var("ETH_HTTP_URL")
            .ok()
            .and_then(|url| url.split("/v2/").nth(1).map(|s| s.to_string()))
            .filter(|k| !k.is_empty() && k != "YOUR_API_KEY")
    }

    /// Get all supported chain configs
    /// CEO Directive: Uses constants for addresses
    pub fn all_chains() -> HashMap<u64, ChainConfig> {
        let mut chains = HashMap::new();
        let alchemy_key = Self::get_alchemy_key();

        for &chain_id in &SUPPORTED_CHAIN_IDS {
            // Get WETH address from constants
            let weth = match get_weth_address(chain_id) {
                Some(addr) => addr,
                None => continue,
            };

            // Get routers from constants
            let routers: Vec<DexRouter> = get_dex_routers(chain_id)
                .into_iter()
                .filter_map(|r| {
                    Address::from_str(r.address).ok().map(|addr| DexRouter {
                        name: r.name.to_string(),
                        address: addr,
                    })
                })
                .collect();

            // Build RPC URL
            let env_key = match chain_id {
                CHAIN_ID_ETHEREUM => "ETH_HTTP_URL",
                CHAIN_ID_BSC => "BSC_HTTP_URL",
                CHAIN_ID_POLYGON => "POLYGON_HTTP_URL",
                CHAIN_ID_ARBITRUM => "ARBITRUM_HTTP_URL",
                CHAIN_ID_OPTIMISM => "OPTIMISM_HTTP_URL",
                CHAIN_ID_AVALANCHE => "AVALANCHE_HTTP_URL",
                CHAIN_ID_BASE => "BASE_HTTP_URL",
                _ => "",
            };

            let rpc_url = std::env::var(env_key)
                .ok()
                .or_else(|| alchemy_key.as_ref().and_then(|k| build_alchemy_url(chain_id, k)))
                .or_else(|| get_public_rpc_fallback(chain_id).map(String::from))
                .unwrap_or_default();

            chains.insert(
                chain_id,
                ChainConfig {
                    chain_id: ChainId::from_id(chain_id).unwrap_or(ChainId::Ethereum),
                    name: get_chain_name(chain_id).to_string(),
                    symbol: get_native_symbol(chain_id).to_string(),
                    weth,
                    routers,
                    rpc_url,
                },
            );
        }

        chains
    }

    /// Get config for specific chain
    pub fn get(chain_id: u64) -> Option<ChainConfig> {
        Self::all_chains().remove(&chain_id)
    }

    /// Get default (Ethereum)
    pub fn default_chain() -> ChainConfig {
        Self::get(CHAIN_ID_ETHEREUM).unwrap()
    }

    /// Get primary router (first in list)
    pub fn primary_router(&self) -> Address {
        self.routers.first().map(|r| r.address).unwrap_or_default()
    }
}

/// Known DEX Router addresses on Ethereum Mainnet
pub struct DexRouters {
    pub addresses: HashSet<Address>,
}

impl Default for DexRouters {
    fn default() -> Self {
        let mut addresses = HashSet::new();

        // Get routers from constants for Ethereum
        for router in get_dex_routers(CHAIN_ID_ETHEREUM) {
            if let Ok(addr) = Address::from_str(router.address) {
                addresses.insert(addr);
            }
        }

        // Add Uniswap V3 routers (not in V2 constants)
        if let Ok(addr) = Address::from_str("0xE592427A0AEce92De3Edee1F18E0157C05861564") {
            addresses.insert(addr);
        }
        if let Ok(addr) = Address::from_str("0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45") {
            addresses.insert(addr);
        }
        // 1inch Router V5
        if let Ok(addr) = Address::from_str("0x1111111254EEB25477B68fb85Ed929f73A960582") {
            addresses.insert(addr);
        }
        // PancakeSwap on ETH
        if let Ok(addr) = Address::from_str("0xEfF92A263d31888d860bD50809A8D171709b7b1c") {
            addresses.insert(addr);
        }

        Self { addresses }
    }
}

impl DexRouters {
    /// Check if an address is a known DEX router
    #[inline]
    pub fn is_dex_router(&self, address: &Address) -> bool {
        self.addresses.contains(address)
    }
}

/// Configuration for the Mempool Sentry
pub struct SentryConfig {
    /// WebSocket URL for the Ethereum node
    pub wss_url: String,
    /// HTTP RPC URL for state fetching
    pub http_url: String,
    /// Maximum concurrent transaction processing
    pub max_concurrent_tasks: usize,
    /// Timeout for RPC calls
    pub rpc_timeout: Duration,
    /// Minimum gas price to consider (filter spam)
    pub min_gas_price_gwei: u64,
    /// Slippage threshold for risk detection (in basis points)
    pub slippage_threshold_bps: u64,
    /// High tax threshold (in basis points)
    pub high_tax_threshold_bps: u64,
}

impl Default for SentryConfig {
    fn default() -> Self {
        Self {
            wss_url: std::env::var("ETH_WSS_URL")
                .unwrap_or_else(|_| "wss://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY".to_string()),
            http_url: std::env::var("ETH_HTTP_URL").unwrap_or_else(|_| {
                "https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY".to_string()
            }),
            max_concurrent_tasks: 50,
            rpc_timeout: Duration::from_secs(5),
            min_gas_price_gwei: 1,
            slippage_threshold_bps: 300,
            high_tax_threshold_bps: 500,
        }
    }
}

impl Clone for SentryConfig {
    fn clone(&self) -> Self {
        Self {
            wss_url: self.wss_url.clone(),
            http_url: self.http_url.clone(),
            max_concurrent_tasks: self.max_concurrent_tasks,
            rpc_timeout: self.rpc_timeout,
            min_gas_price_gwei: self.min_gas_price_gwei,
            slippage_threshold_bps: self.slippage_threshold_bps,
            high_tax_threshold_bps: self.high_tax_threshold_bps,
        }
    }
}
