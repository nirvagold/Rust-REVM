//! Constants Module - Single Source of Truth
//!
//! CEO Directive: Semua konstanta, fungsi konversi, dan konfigurasi
//! yang digunakan di seluruh aplikasi HARUS didefinisikan di sini.
//! Tidak ada hardcoded values di modul lain!

use alloy_primitives::{Address, U256};
use std::str::FromStr;

// ============================================
// APPLICATION CONSTANTS
// ============================================

/// Application name
pub const APP_NAME: &str = "RusterShield";

/// Application version
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// User-Agent for HTTP requests (CEO Directive: Alchemy dashboard monitoring)
pub const USER_AGENT: &str = "RusterShield/1.0.0";

// ============================================
// RPC CONSTANTS
// ============================================

/// Default timeout for RPC requests (seconds)
pub const DEFAULT_RPC_TIMEOUT_SECS: u64 = 10;

/// Default cache TTL (seconds)
pub const DEFAULT_CACHE_TTL_SECS: u64 = 300;

// Note: Retry constants moved to src/providers/rpc.rs (Alchemy Best Practices)
// - ALCHEMY_BASE_RETRY_MS = 1000 (start at 1 second)
// - ALCHEMY_MAX_RETRY_MS = 64000 (cap at 64 seconds)
// - ALCHEMY_MAX_RETRIES = 7 (exponential: 1s→2s→4s→8s→16s→32s→64s)

// ============================================
// CHAIN IDS - Single Source of Truth
// ============================================

/// Ethereum Mainnet
pub const CHAIN_ID_ETHEREUM: u64 = 1;
/// BNB Smart Chain
pub const CHAIN_ID_BSC: u64 = 56;
/// Polygon
pub const CHAIN_ID_POLYGON: u64 = 137;
/// Arbitrum One
pub const CHAIN_ID_ARBITRUM: u64 = 42161;
/// Optimism
pub const CHAIN_ID_OPTIMISM: u64 = 10;
/// Avalanche C-Chain
pub const CHAIN_ID_AVALANCHE: u64 = 43114;
/// Base
pub const CHAIN_ID_BASE: u64 = 8453;
/// Solana (non-EVM, special handling)
pub const CHAIN_ID_SOLANA: u64 = 900; // Custom ID for Solana

/// All supported EVM chain IDs
pub const SUPPORTED_CHAIN_IDS: [u64; 7] = [
    CHAIN_ID_ETHEREUM,
    CHAIN_ID_BSC,
    CHAIN_ID_POLYGON,
    CHAIN_ID_ARBITRUM,
    CHAIN_ID_OPTIMISM,
    CHAIN_ID_AVALANCHE,
    CHAIN_ID_BASE,
];

/// All supported chain IDs including Solana
pub const ALL_SUPPORTED_CHAINS: [u64; 8] = [
    CHAIN_ID_ETHEREUM,
    CHAIN_ID_BSC,
    CHAIN_ID_POLYGON,
    CHAIN_ID_ARBITRUM,
    CHAIN_ID_OPTIMISM,
    CHAIN_ID_AVALANCHE,
    CHAIN_ID_BASE,
    CHAIN_ID_SOLANA,
];

// ============================================
// WETH/WBNB ADDRESSES - Single Source of Truth
// ============================================

/// Get WETH/WBNB address for a chain
pub fn get_weth_address(chain_id: u64) -> Option<Address> {
    let addr_str = match chain_id {
        CHAIN_ID_ETHEREUM => "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
        CHAIN_ID_BSC => "0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c",
        CHAIN_ID_POLYGON => "0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270",
        CHAIN_ID_ARBITRUM => "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1",
        CHAIN_ID_OPTIMISM => "0x4200000000000000000000000000000000000006",
        CHAIN_ID_AVALANCHE => "0xB31f66AA3C1e785363F0875A1B74E27b85FD66c7",
        CHAIN_ID_BASE => "0x4200000000000000000000000000000000000006",
        _ => return None,
    };
    Address::from_str(addr_str).ok()
}

// ============================================
// DEX ROUTER ADDRESSES - Single Source of Truth
// ============================================

/// DEX Router info
#[derive(Debug, Clone)]
pub struct RouterInfo {
    pub name: &'static str,
    pub address: &'static str,
}

/// Get DEX routers for a chain (V2 compatible only)
pub fn get_dex_routers(chain_id: u64) -> Vec<RouterInfo> {
    match chain_id {
        CHAIN_ID_ETHEREUM => vec![
            RouterInfo { name: "Uniswap V2", address: "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D" },
            RouterInfo { name: "SushiSwap", address: "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F" },
        ],
        CHAIN_ID_BSC => vec![
            RouterInfo { name: "PancakeSwap V2", address: "0x10ED43C718714eb63d5aA57B78B54704E256024E" },
            RouterInfo { name: "BiSwap", address: "0x3a6d8cA21D1CF76F653A67577FA0D27453350dD8" },
        ],
        CHAIN_ID_POLYGON => vec![
            RouterInfo { name: "QuickSwap", address: "0xa5E0829CaCEd8fFDD4De3c43696c57F7D7A678ff" },
            RouterInfo { name: "SushiSwap", address: "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506" },
        ],
        CHAIN_ID_ARBITRUM => vec![
            RouterInfo { name: "Camelot", address: "0xc873fEcbd354f5A56E00E710B90EF4201db2448d" },
            RouterInfo { name: "SushiSwap", address: "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506" },
        ],
        CHAIN_ID_OPTIMISM => vec![
            RouterInfo { name: "SushiSwap", address: "0x4C5D5234f232BD2D76B96aA33F5AE4FCF0E4BFAb" },
        ],
        CHAIN_ID_AVALANCHE => vec![
            RouterInfo { name: "TraderJoe", address: "0x60aE616a2155Ee3d9A68541Ba4544862310933d4" },
            RouterInfo { name: "Pangolin", address: "0xE54Ca86531e17Ef3616d22Ca28b0D458b6C89106" },
        ],
        CHAIN_ID_BASE => vec![
            RouterInfo { name: "PancakeSwap V2", address: "0x02a84c1b3BBD7401a5f7fa98a384EBC70bB5749E" },
            RouterInfo { name: "BaseSwap", address: "0x327Df1E6de05895d2ab08513aaDD9313Fe505d86" },
            RouterInfo { name: "SushiSwap", address: "0x6BDED42c6DA8FBf0d2bA55B2fa120C5e0c8D7891" },
        ],
        _ => vec![],
    }
}

// ============================================
// PUBLIC RPC FALLBACKS - Single Source of Truth
// ============================================

/// Get public RPC fallback URL for a chain
pub fn get_public_rpc_fallback(chain_id: u64) -> Option<&'static str> {
    match chain_id {
        CHAIN_ID_ETHEREUM => Some("https://eth.llamarpc.com"),
        CHAIN_ID_BSC => Some("https://bsc-dataseed.binance.org"),
        CHAIN_ID_POLYGON => Some("https://polygon-rpc.com"),
        CHAIN_ID_ARBITRUM => Some("https://arb1.arbitrum.io/rpc"),
        CHAIN_ID_OPTIMISM => Some("https://mainnet.optimism.io"),
        CHAIN_ID_AVALANCHE => Some("https://api.avax.network/ext/bc/C/rpc"),
        CHAIN_ID_BASE => Some("https://mainnet.base.org"),
        _ => None,
    }
}

// ============================================
// ALCHEMY NETWORK MAPPING
// ============================================

/// Get Alchemy subdomain for a chain
pub fn get_alchemy_subdomain(chain_id: u64) -> Option<&'static str> {
    match chain_id {
        CHAIN_ID_ETHEREUM => Some("eth-mainnet"),
        CHAIN_ID_BSC => Some("bnb-mainnet"),
        CHAIN_ID_POLYGON => Some("polygon-mainnet"),
        CHAIN_ID_ARBITRUM => Some("arb-mainnet"),
        CHAIN_ID_OPTIMISM => Some("opt-mainnet"),
        CHAIN_ID_AVALANCHE => Some("avax-mainnet"),
        CHAIN_ID_BASE => Some("base-mainnet"),
        _ => None,
    }
}

/// Build Alchemy URL for a chain
pub fn build_alchemy_url(chain_id: u64, api_key: &str) -> Option<String> {
    get_alchemy_subdomain(chain_id)
        .map(|subdomain| format!("https://{}.g.alchemy.com/v2/{}", subdomain, api_key))
}

// ============================================
// CHAIN METADATA
// ============================================

/// Get chain name
pub fn get_chain_name(chain_id: u64) -> &'static str {
    match chain_id {
        CHAIN_ID_ETHEREUM => "Ethereum",
        CHAIN_ID_BSC => "BNB Smart Chain",
        CHAIN_ID_POLYGON => "Polygon",
        CHAIN_ID_ARBITRUM => "Arbitrum One",
        CHAIN_ID_OPTIMISM => "Optimism",
        CHAIN_ID_AVALANCHE => "Avalanche C-Chain",
        CHAIN_ID_BASE => "Base",
        CHAIN_ID_SOLANA => "Solana",
        _ => "Unknown",
    }
}

/// Get native token symbol
pub fn get_native_symbol(chain_id: u64) -> &'static str {
    match chain_id {
        CHAIN_ID_ETHEREUM => "ETH",
        CHAIN_ID_BSC => "BNB",
        CHAIN_ID_POLYGON => "MATIC",
        CHAIN_ID_ARBITRUM => "ETH",
        CHAIN_ID_OPTIMISM => "ETH",
        CHAIN_ID_AVALANCHE => "AVAX",
        CHAIN_ID_BASE => "ETH",
        CHAIN_ID_SOLANA => "SOL",
        _ => "ETH",
    }
}

/// Get block explorer URL
pub fn get_explorer_url(chain_id: u64) -> &'static str {
    match chain_id {
        CHAIN_ID_ETHEREUM => "https://etherscan.io",
        CHAIN_ID_BSC => "https://bscscan.com",
        CHAIN_ID_POLYGON => "https://polygonscan.com",
        CHAIN_ID_ARBITRUM => "https://arbiscan.io",
        CHAIN_ID_OPTIMISM => "https://optimistic.etherscan.io",
        CHAIN_ID_AVALANCHE => "https://snowtrace.io",
        CHAIN_ID_BASE => "https://basescan.org",
        _ => "https://etherscan.io",
    }
}

// ============================================
// CONVERSION UTILITIES - Single Source of Truth
// ============================================

/// Convert wei to ETH (or native token)
/// CEO Directive: This is THE ONLY place this function should exist!
#[inline]
pub fn wei_to_eth(wei: U256) -> f64 {
    let wei_u128: u128 = wei.try_into().unwrap_or(u128::MAX);
    wei_u128 as f64 / 1e18
}

/// Convert ETH to wei
#[inline]
pub fn eth_to_wei(eth: f64) -> U256 {
    U256::from((eth * 1e18) as u128)
}

/// Check if chain ID is supported
#[inline]
pub fn is_chain_supported(chain_id: u64) -> bool {
    SUPPORTED_CHAIN_IDS.contains(&chain_id)
}

// ============================================
// DEXSCREENER CHAIN MAPPING
// ============================================

/// Convert numeric chain ID to DexScreener chain name
pub fn chain_id_to_dexscreener_name(chain_id: u64) -> &'static str {
    match chain_id {
        CHAIN_ID_ETHEREUM => "ethereum",
        CHAIN_ID_BSC => "bsc",
        CHAIN_ID_POLYGON => "polygon",
        CHAIN_ID_ARBITRUM => "arbitrum",
        CHAIN_ID_OPTIMISM => "optimism",
        CHAIN_ID_AVALANCHE => "avalanche",
        CHAIN_ID_BASE => "base",
        CHAIN_ID_SOLANA => "solana",
        _ => "ethereum",
    }
}

/// Convert DexScreener chain name to numeric chain ID
pub fn dexscreener_name_to_chain_id(name: &str) -> u64 {
    match name.to_lowercase().as_str() {
        "ethereum" => CHAIN_ID_ETHEREUM,
        "bsc" => CHAIN_ID_BSC,
        "polygon" => CHAIN_ID_POLYGON,
        "arbitrum" => CHAIN_ID_ARBITRUM,
        "optimism" => CHAIN_ID_OPTIMISM,
        "avalanche" => CHAIN_ID_AVALANCHE,
        "base" => CHAIN_ID_BASE,
        "solana" => CHAIN_ID_SOLANA,
        _ => CHAIN_ID_ETHEREUM,
    }
}

/// Check if chain is Solana (non-EVM)
pub fn is_solana(chain_id: u64) -> bool {
    chain_id == CHAIN_ID_SOLANA
}

/// Check if address looks like Solana (base58, 32-44 chars, no 0x prefix)
pub fn is_solana_address(address: &str) -> bool {
    // Solana addresses are base58 encoded, 32-44 characters, no 0x prefix
    !address.starts_with("0x") 
        && address.len() >= 32 
        && address.len() <= 44
        && address.chars().all(|c| c.is_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wei_to_eth() {
        let one_eth = U256::from(1_000_000_000_000_000_000u128);
        assert!((wei_to_eth(one_eth) - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_eth_to_wei() {
        let wei = eth_to_wei(1.5);
        assert_eq!(wei, U256::from(1_500_000_000_000_000_000u128));
    }

    #[test]
    fn test_chain_support() {
        assert!(is_chain_supported(1));
        assert!(is_chain_supported(56));
        assert!(!is_chain_supported(999));
    }

    #[test]
    fn test_weth_addresses() {
        assert!(get_weth_address(1).is_some());
        assert!(get_weth_address(999).is_none());
    }
}
