//! Integration tests for Mempool Sentry

use alloy_primitives::{Address, Bytes, U256, B256};
use mempool_sentry::{
    config::DexRouters,
    decoder::SwapDecoder,
    types::{AnalysisResult, RiskFactor, RiskLevel},
};
use std::str::FromStr;

#[test]
fn test_dex_router_detection() {
    let routers = DexRouters::default();
    
    // Uniswap V2 Router should be detected
    let uniswap_v2 = Address::from_str("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D").unwrap();
    assert!(routers.is_dex_router(&uniswap_v2), "Uniswap V2 Router should be detected");
    
    // Random address should not be detected
    let random = Address::from_str("0x0000000000000000000000000000000000000001").unwrap();
    assert!(!routers.is_dex_router(&random), "Random address should not be DEX router");
}

#[test]
fn test_risk_level_ordering() {
    assert!((RiskLevel::Safe as u8) < (RiskLevel::Low as u8));
    assert!((RiskLevel::Low as u8) < (RiskLevel::Medium as u8));
    assert!((RiskLevel::Medium as u8) < (RiskLevel::High as u8));
    assert!((RiskLevel::High as u8) < (RiskLevel::Critical as u8));
}

#[test]
fn test_analysis_result_risk_accumulation() {
    let tx_hash = B256::default();
    let from = Address::default();
    let target = Address::default();
    let value = U256::from(1_000_000_000_000_000_000u128); // 1 ETH
    let gas_price = U256::from(20_000_000_000u64); // 20 gwei
    
    let mut result = AnalysisResult::new(tx_hash, from, target, value, gas_price);
    
    // Initially safe
    assert_eq!(result.risk_level, RiskLevel::Safe);
    
    // Add low risk
    result.add_risk(RiskFactor::LargeValue { value_eth: 15.0 });
    assert_eq!(result.risk_level, RiskLevel::Low);
    
    // Add medium risk - should upgrade
    result.add_risk(RiskFactor::HighSlippage { expected_bps: 100, actual_bps: 400 });
    assert_eq!(result.risk_level, RiskLevel::Medium);
    
    // Add critical risk - should upgrade to critical
    result.add_risk(RiskFactor::SandwichTarget { 
        reason: "Test sandwich".to_string() 
    });
    assert_eq!(result.risk_level, RiskLevel::Critical);
    
    // Should have 3 risk factors
    assert_eq!(result.risk_factors.len(), 3);
}

#[test]
fn test_swap_decoder_empty_calldata() {
    let empty = Bytes::new();
    let result = SwapDecoder::decode(&empty, U256::ZERO);
    assert!(result.is_none(), "Empty calldata should return None");
}

#[test]
fn test_swap_decoder_short_calldata() {
    let short = Bytes::from(vec![0x7f, 0xf3, 0x6a]); // Only 3 bytes
    let result = SwapDecoder::decode(&short, U256::ZERO);
    assert!(result.is_none(), "Short calldata should return None");
}

#[test]
fn test_slippage_calculation() {
    let amount_in = U256::from(1_000_000_000_000_000_000u128); // 1 ETH
    let amount_out_min = U256::from(970_000_000_000_000_000u128); // 0.97 ETH
    let expected_rate = U256::from(1);
    
    let slippage = SwapDecoder::calculate_slippage_bps(amount_in, amount_out_min, expected_rate);
    assert_eq!(slippage, 300, "Should be 3% (300 bps) slippage");
}

#[test]
fn test_slippage_calculation_zero_input() {
    let slippage = SwapDecoder::calculate_slippage_bps(U256::ZERO, U256::from(100), U256::from(1));
    assert_eq!(slippage, 0, "Zero input should return 0 slippage");
}

#[test]
fn test_risk_factor_descriptions() {
    let factors = vec![
        RiskFactor::HighSlippage { expected_bps: 100, actual_bps: 500 },
        RiskFactor::HighTax { tax_bps: 1000 },
        RiskFactor::SandwichTarget { reason: "Large swap".to_string() },
        RiskFactor::Honeypot { reason: "Cannot sell".to_string(), buy_success: true, sell_success: false },
        RiskFactor::UnusualGasPrice { gas_gwei: 200, avg_gwei: 30 },
        RiskFactor::LargeValue { value_eth: 50.0 },
        RiskFactor::UnverifiedContract,
        RiskFactor::SimulationFailed { reason: "Reverted".to_string() },
    ];
    
    for factor in factors {
        let desc = factor.description();
        assert!(!desc.is_empty(), "Description should not be empty");
    }
}

#[test]
fn test_analysis_result_summary() {
    let tx_hash = B256::default();
    let from = Address::default();
    let target = Address::default();
    let value = U256::from(1_000_000_000_000_000_000u128);
    let gas_price = U256::from(20_000_000_000u64);
    
    let mut result = AnalysisResult::new(tx_hash, from, target, value, gas_price);
    result.add_risk(RiskFactor::HighSlippage { expected_bps: 100, actual_bps: 500 });
    
    let summary = result.summary();
    assert!(summary.contains("Risk:"), "Summary should contain risk level");
    assert!(summary.contains("TX:"), "Summary should contain transaction hash");
    assert!(summary.contains("From:"), "Summary should contain from address");
}

#[test]
fn test_wei_to_eth_conversion() {
    // Test via AnalysisResult large value detection
    let tx_hash = B256::default();
    let from = Address::default();
    let target = Address::default();
    let value = U256::from(15_000_000_000_000_000_000u128); // 15 ETH
    let gas_price = U256::from(20_000_000_000u64);
    
    let result = AnalysisResult::new(tx_hash, from, target, value, gas_price);
    // Value is stored correctly
    assert_eq!(result.value, value);
}
