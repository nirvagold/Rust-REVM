//! Configuration module for Mempool Sentry
//! Handles all configurable parameters and known DEX router addresses

use alloy_primitives::Address;
use std::collections::HashSet;
use std::str::FromStr;
use std::time::Duration;

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
            http_url: std::env::var("ETH_HTTP_URL")
                .unwrap_or_else(|_| "https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY".to_string()),
            max_concurrent_tasks: 50,
            rpc_timeout: Duration::from_secs(5),
            min_gas_price_gwei: 1,
            slippage_threshold_bps: 300,  // 3% slippage = risky
            high_tax_threshold_bps: 500,  // 5% tax = high tax token
        }
    }
}
