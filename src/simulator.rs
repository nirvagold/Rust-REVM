//! REVM-based transaction simulator
//! Simulates transactions in-memory to detect risks before on-chain execution

#![allow(dead_code)]

use alloy_primitives::{Address, Bytes, B256, U256};
use eyre::Result;
use revm::{
    db::{CacheDB, EmptyDB},
    primitives::{
        AccountInfo, BlockEnv, CfgEnvWithHandlerCfg, EnvWithHandlerCfg, ExecutionResult, Output,
        SpecId, TxEnv, TxKind,
    },
    Evm,
};
use std::collections::HashMap;

use crate::types::{RiskFactor, SwapParams};

/// Simulation result containing execution outcome and detected risks
#[derive(Debug)]
pub struct SimulationResult {
    pub success: bool,
    pub gas_used: u64,
    pub output: Vec<u8>,
    pub risks: Vec<RiskFactor>,
    pub balance_changes: HashMap<Address, BalanceChange>,
}

/// Balance change for an address
#[derive(Debug, Clone)]
pub struct BalanceChange {
    pub before: U256,
    pub after: U256,
}

impl BalanceChange {
    pub fn diff(&self) -> i128 {
        let before: u128 = self.before.try_into().unwrap_or(u128::MAX);
        let after: u128 = self.after.try_into().unwrap_or(u128::MAX);
        after as i128 - before as i128
    }
}

/// Transaction simulator using REVM
pub struct Simulator {
    /// Chain ID
    chain_id: u64,
}

impl Simulator {
    /// Create a new simulator instance
    pub fn new(chain_id: u64) -> Self {
        Self { chain_id }
    }

    /// Simulate a transaction and return the result
    #[allow(clippy::too_many_arguments)]
    pub fn simulate(
        &self,
        from: Address,
        to: Option<Address>,
        value: U256,
        gas_limit: u64,
        gas_price: u128,
        input: Bytes,
        nonce: u64,
        swap_params: Option<&SwapParams>,
    ) -> Result<SimulationResult> {
        let mut risks = Vec::new();
        let mut db = CacheDB::new(EmptyDB::default());

        // Load minimal account state
        db.insert_account_info(
            from,
            AccountInfo {
                balance: U256::from(100_000_000_000_000_000_000u128), // 100 ETH for simulation
                nonce,
                code_hash: B256::default(),
                code: None,
            },
        );

        // Record balances before
        let from_balance_before = db
            .accounts
            .get(&from)
            .map(|a| a.info.balance)
            .unwrap_or_default();
        let to_balance_before = to
            .and_then(|t| db.accounts.get(&t).map(|a| a.info.balance))
            .unwrap_or_default();

        // Build transaction environment
        let transact_to = match to {
            Some(addr) => TxKind::Call(addr),
            None => TxKind::Create,
        };

        let tx_env = TxEnv {
            caller: from,
            gas_limit,
            gas_price: U256::from(gas_price),
            transact_to,
            value,
            data: input,
            nonce: Some(nonce),
            chain_id: Some(self.chain_id),
            ..Default::default()
        };

        // Build block environment
        let block_env = BlockEnv {
            number: U256::from(19_000_000u64),
            timestamp: U256::from(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            ),
            gas_limit: U256::from(30_000_000u64),
            basefee: U256::from(20_000_000_000u64),
            ..Default::default()
        };

        // Build combined environment
        let cfg = CfgEnvWithHandlerCfg::new_with_spec_id(Default::default(), SpecId::CANCUN);
        let env = EnvWithHandlerCfg::new_with_cfg_env(cfg, block_env, tx_env);

        // Create EVM instance and execute
        let mut evm = Evm::builder()
            .with_db(&mut db)
            .with_env_with_handler_cfg(env)
            .build();

        let result = evm.transact();

        // Drop EVM to release borrow
        drop(evm);

        let (success, gas_used, output) = match result {
            Ok(result_and_state) => match result_and_state.result {
                ExecutionResult::Success {
                    gas_used, output, ..
                } => {
                    let output_bytes = match output {
                        Output::Call(bytes) => bytes.to_vec(),
                        Output::Create(bytes, _) => bytes.to_vec(),
                    };
                    (true, gas_used, output_bytes)
                }
                ExecutionResult::Revert { gas_used, output } => {
                    risks.push(RiskFactor::SimulationFailed {
                        reason: format!("Reverted: {}", hex::encode(&output)),
                    });
                    (false, gas_used, output.to_vec())
                }
                ExecutionResult::Halt { reason, gas_used } => {
                    risks.push(RiskFactor::SimulationFailed {
                        reason: format!("Halted: {:?}", reason),
                    });
                    (false, gas_used, Vec::new())
                }
            },
            Err(e) => {
                risks.push(RiskFactor::SimulationFailed {
                    reason: format!("EVM error: {:?}", e),
                });
                (false, 0, Vec::new())
            }
        };

        // Analyze swap parameters for slippage risk
        if let Some(params) = swap_params {
            analyze_swap_risks(params, &mut risks);
        }

        // Check for large value transactions
        let value_eth = wei_to_eth(value);
        if value_eth > 10.0 {
            risks.push(RiskFactor::LargeValue { value_eth });
        }

        // Build balance changes map
        let mut balance_changes = HashMap::new();
        let from_balance_after = db
            .accounts
            .get(&from)
            .map(|a| a.info.balance)
            .unwrap_or_default();
        balance_changes.insert(
            from,
            BalanceChange {
                before: from_balance_before,
                after: from_balance_after,
            },
        );

        if let Some(to_addr) = to {
            let to_balance_after = db
                .accounts
                .get(&to_addr)
                .map(|a| a.info.balance)
                .unwrap_or_default();
            balance_changes.insert(
                to_addr,
                BalanceChange {
                    before: to_balance_before,
                    after: to_balance_after,
                },
            );
        }

        Ok(SimulationResult {
            success,
            gas_used,
            output,
            risks,
            balance_changes,
        })
    }
}

/// Analyze swap parameters for potential risks (standalone function)
fn analyze_swap_risks(params: &SwapParams, risks: &mut Vec<RiskFactor>) {
    // Check for extremely low amount_out_min (high slippage tolerance)
    if !params.amount_in.is_zero() {
        let ratio = params
            .amount_out_min
            .saturating_mul(U256::from(10000))
            .checked_div(params.amount_in)
            .unwrap_or_default();

        let ratio_u64: u64 = ratio.try_into().unwrap_or(0);
        if ratio_u64 < 9000 && ratio_u64 > 0 {
            let slippage_bps = 10000 - ratio_u64;
            risks.push(RiskFactor::HighSlippage {
                expected_bps: 100,
                actual_bps: slippage_bps,
            });
        }
    }

    // Check for sandwich attack indicators
    let value_eth = wei_to_eth(params.amount_in);
    if value_eth > 1.0 {
        let slippage_tolerance = if !params.amount_in.is_zero() {
            let ratio = params
                .amount_out_min
                .saturating_mul(U256::from(100))
                .checked_div(params.amount_in)
                .unwrap_or_default();
            100u64.saturating_sub(ratio.try_into().unwrap_or(100))
        } else {
            0
        };

        if slippage_tolerance > 5 {
            risks.push(RiskFactor::SandwichTarget {
                reason: format!(
                    "Large swap ({:.2} ETH) with {}% slippage tolerance",
                    value_eth, slippage_tolerance
                ),
            });
        }
    }

    // Check deadline
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let deadline_secs: u64 = params.deadline.try_into().unwrap_or(0);
    if deadline_secs > now + 600 {
        risks.push(RiskFactor::SandwichTarget {
            reason: "Long deadline window increases MEV exposure".to_string(),
        });
    }
}

/// Convert wei to ETH
fn wei_to_eth(wei: U256) -> f64 {
    let wei_u128: u128 = wei.try_into().unwrap_or(u128::MAX);
    wei_u128 as f64 / 1e18
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wei_to_eth() {
        let one_eth = U256::from(1_000_000_000_000_000_000u128);
        assert!((wei_to_eth(one_eth) - 1.0).abs() < 0.0001);
    }
}
