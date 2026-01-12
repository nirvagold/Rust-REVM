//! Configuration module for Mempool Sentry
//! Handles all configurable parameters and known DEX router addresses

use alloy_primitives::Address;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::time::Duration;

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
    #[allow(dead_code)]
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

    /// Get chain name
    #[allow(dead_code)]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Ethereum => "Ethereum",
            Self::BinanceSmartChain => "BNB Smart Chain",
            Self::Polygon => "Polygon",
            Self::Arbitrum => "Arbitrum One",
            Self::Optimism => "Optimism",
            Self::Avalanche => "Avalanche C-Chain",
            Self::Base => "Base",
        }
    }

    /// Get chain symbol
    #[allow(dead_code)]
    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Ethereum => "ETH",
            Self::BinanceSmartChain => "BNB",
            Self::Polygon => "MATIC",
            Self::Arbitrum => "ETH",
            Self::Optimism => "ETH",
            Self::Avalanche => "AVAX",
            Self::Base => "ETH",
        }
    }

    /// Get block explorer URL
    #[allow(dead_code)]
    pub fn explorer(&self) -> &'static str {
        match self {
            Self::Ethereum => "https://etherscan.io",
            Self::BinanceSmartChain => "https://bscscan.com",
            Self::Polygon => "https://polygonscan.com",
            Self::Arbitrum => "https://arbiscan.io",
            Self::Optimism => "https://optimistic.etherscan.io",
            Self::Avalanche => "https://snowtrace.io",
            Self::Base => "https://basescan.org",
        }
    }
}

/// Chain-specific configuration (WETH, Router, RPC)
#[derive(Debug, Clone)]
pub struct ChainConfig {
    pub chain_id: ChainId,
    pub name: String,
    pub symbol: String,
    pub weth: Address,
    pub router: Address,
    pub rpc_url: String,
}

impl ChainConfig {
    /// Get Alchemy RPC URL for a chain (if supported)
    fn alchemy_url(chain_id: u64, api_key: &str) -> Option<String> {
        let network = match chain_id {
            1 => "eth-mainnet",
            137 => "polygon-mainnet",
            42161 => "arb-mainnet",
            10 => "opt-mainnet",
            8453 => "base-mainnet",
            _ => return None, // BSC & Avalanche not supported by Alchemy
        };
        Some(format!("https://{}.g.alchemy.com/v2/{}", network, api_key))
    }

    /// Get all supported chain configs
    pub fn all_chains() -> HashMap<u64, ChainConfig> {
        let mut chains = HashMap::new();
        
        // Try to get Alchemy API key from ETH_HTTP_URL
        let alchemy_key = std::env::var("ETH_HTTP_URL")
            .ok()
            .and_then(|url| {
                // Extract API key from URL like https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY
                url.split("/v2/").nth(1).map(|s| s.to_string())
            });

        // Ethereum Mainnet
        let eth_rpc = std::env::var("ETH_HTTP_URL")
            .unwrap_or_else(|_| "https://eth.llamarpc.com".to_string());
        chains.insert(1, ChainConfig {
            chain_id: ChainId::Ethereum,
            name: "Ethereum".to_string(),
            symbol: "ETH".to_string(),
            weth: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap(),
            router: "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap(), // Uniswap V2
            rpc_url: eth_rpc,
        });

        // BNB Smart Chain (Alchemy doesn't support BSC)
        let bsc_rpc = std::env::var("BSC_HTTP_URL")
            .unwrap_or_else(|_| "https://bsc-dataseed.binance.org".to_string());
        chains.insert(56, ChainConfig {
            chain_id: ChainId::BinanceSmartChain,
            name: "BNB Smart Chain".to_string(),
            symbol: "BNB".to_string(),
            weth: "0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c".parse().unwrap(), // WBNB
            router: "0x10ED43C718714eb63d5aA57B78B54704E256024E".parse().unwrap(), // PancakeSwap V2
            rpc_url: bsc_rpc,
        });

        // Polygon - Use Alchemy if available
        let polygon_rpc = std::env::var("POLYGON_HTTP_URL")
            .ok()
            .or_else(|| alchemy_key.as_ref().and_then(|k| Self::alchemy_url(137, k)))
            .unwrap_or_else(|| "https://polygon-rpc.com".to_string());
        chains.insert(137, ChainConfig {
            chain_id: ChainId::Polygon,
            name: "Polygon".to_string(),
            symbol: "MATIC".to_string(),
            weth: "0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270".parse().unwrap(), // WMATIC
            router: "0xa5E0829CaCEd8fFDD4De3c43696c57F7D7A678ff".parse().unwrap(), // QuickSwap
            rpc_url: polygon_rpc,
        });

        // Arbitrum One - Use Alchemy if available
        let arb_rpc = std::env::var("ARBITRUM_HTTP_URL")
            .ok()
            .or_else(|| alchemy_key.as_ref().and_then(|k| Self::alchemy_url(42161, k)))
            .unwrap_or_else(|| "https://arb1.arbitrum.io/rpc".to_string());
        chains.insert(42161, ChainConfig {
            chain_id: ChainId::Arbitrum,
            name: "Arbitrum One".to_string(),
            symbol: "ETH".to_string(),
            weth: "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1".parse().unwrap(), // WETH on Arb
            router: "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506".parse().unwrap(), // SushiSwap
            rpc_url: arb_rpc,
        });

        // Optimism - Use Alchemy if available
        let op_rpc = std::env::var("OPTIMISM_HTTP_URL")
            .ok()
            .or_else(|| alchemy_key.as_ref().and_then(|k| Self::alchemy_url(10, k)))
            .unwrap_or_else(|| "https://mainnet.optimism.io".to_string());
        chains.insert(10, ChainConfig {
            chain_id: ChainId::Optimism,
            name: "Optimism".to_string(),
            symbol: "ETH".to_string(),
            weth: "0x4200000000000000000000000000000000000006".parse().unwrap(), // WETH on OP
            router: "0x9c12939390052919aF3155f41Bf4160Fd3666A6f".parse().unwrap(), // Velodrome
            rpc_url: op_rpc,
        });

        // Avalanche C-Chain (Alchemy doesn't support Avalanche)
        let avax_rpc = std::env::var("AVALANCHE_HTTP_URL")
            .unwrap_or_else(|_| "https://api.avax.network/ext/bc/C/rpc".to_string());
        chains.insert(43114, ChainConfig {
            chain_id: ChainId::Avalanche,
            name: "Avalanche C-Chain".to_string(),
            symbol: "AVAX".to_string(),
            weth: "0xB31f66AA3C1e785363F0875A1B74E27b85FD66c7".parse().unwrap(), // WAVAX
            router: "0x60aE616a2155Ee3d9A68541Ba4544862310933d4".parse().unwrap(), // TraderJoe
            rpc_url: avax_rpc,
        });

        // Base - Use Alchemy if available
        let base_rpc = std::env::var("BASE_HTTP_URL")
            .ok()
            .or_else(|| alchemy_key.as_ref().and_then(|k| Self::alchemy_url(8453, k)))
            .unwrap_or_else(|| "https://mainnet.base.org".to_string());
        chains.insert(8453, ChainConfig {
            chain_id: ChainId::Base,
            name: "Base".to_string(),
            symbol: "ETH".to_string(),
            weth: "0x4200000000000000000000000000000000000006".parse().unwrap(), // WETH on Base
            router: "0x8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb".parse().unwrap(), // BaseSwap
            rpc_url: base_rpc,
        });

        chains
    }

    /// Get config for specific chain
    pub fn get(chain_id: u64) -> Option<ChainConfig> {
        Self::all_chains().remove(&chain_id)
    }

    /// Get default (Ethereum)
    #[allow(dead_code)]
    pub fn default_chain() -> ChainConfig {
        Self::get(1).unwrap()
    }
}

/// Known DEX Router addresses on Ethereum Mainnet
/// These are the primary targets for sandwich attack detection
pub struct DexRouters {
    pub addresses: HashSet<Address>,
}

impl Default for DexRouters {
    fn default() -> Self {
        let mut addresses = HashSet::new();

        // Uniswap V2 Router
        if let Ok(addr) = Address::from_str("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D") {
            addresses.insert(addr);
        }

        // Uniswap V3 Router
        if let Ok(addr) = Address::from_str("0xE592427A0AEce92De3Edee1F18E0157C05861564") {
            addresses.insert(addr);
        }

        // Uniswap V3 Router 2
        if let Ok(addr) = Address::from_str("0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45") {
            addresses.insert(addr);
        }

        // SushiSwap Router
        if let Ok(addr) = Address::from_str("0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F") {
            addresses.insert(addr);
        }

        // 1inch Router V5
        if let Ok(addr) = Address::from_str("0x1111111254EEB25477B68fb85Ed929f73A960582") {
            addresses.insert(addr);
        }

        // PancakeSwap Router (on ETH)
        if let Ok(addr) = Address::from_str("0xEfF92A263d31888d860bD50809A8D171709b7b1c") {
            addresses.insert(addr);
        }

        Self { addresses }
    }
}

impl DexRouters {
    /// Check if an address is a known DEX router
    #[inline]
    #[allow(dead_code)]
    pub fn is_dex_router(&self, address: &Address) -> bool {
        self.addresses.contains(address)
    }
}

/// Configuration for the Mempool Sentry
pub struct SentryConfig {
    /// WebSocket URL for the Ethereum node (Alchemy/QuickNode)
    pub wss_url: String,

    /// HTTP RPC URL for state fetching
    pub http_url: String,

    /// Maximum concurrent transaction processing
    pub max_concurrent_tasks: usize,

    /// Timeout for RPC calls
    pub rpc_timeout: Duration,

    /// Minimum gas price to consider (filter spam)
    pub min_gas_price_gwei: u64,

    /// Slippage threshold for risk detection (in basis points, 100 = 1%)
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
            slippage_threshold_bps: 300, // 3% slippage = risky
            high_tax_threshold_bps: 500, // 5% tax = high tax token
        }
    }
}
