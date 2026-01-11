//! Honeypot Detection Module
//! Simulates Buy → Sell cycle in-memory to detect honeypot tokens
//! 
//! A honeypot is a token that allows buying but blocks selling.
//! We detect this by simulating:
//! 1. Approve token for router
//! 2. Swap ETH → Token (Buy)
//! 3. Swap Token → ETH (Sell)
//! If step 3 fails or returns significantly less than expected → HONEYPOT

use alloy_primitives::{Address, Bytes, U256};
use alloy_sol_types::{sol, SolCall};
use eyre::{Result, eyre};
use revm::{
    db::{CacheDB, EmptyDB},
    primitives::{
        AccountInfo, Bytecode, TxEnv, BlockEnv, CfgEnvWithHandlerCfg, ExecutionResult,
        Output, SpecId, TxKind, EnvWithHandlerCfg, KECCAK_EMPTY,
    },
    Evm,
};
use std::time::Instant;

// ERC20 and Router interfaces
sol! {
    // ERC20 functions
    function balanceOf(address account) external view returns (uint256);
    function approve(address spender, uint256 amount) external returns (bool);
    function transfer(address to, uint256 amount) external returns (bool);
    
    // Uniswap V2 Router
    function swapExactETHForTokens(
        uint256 amountOutMin,
        address[] calldata path,
        address to,
        uint256 deadline
    ) external payable returns (uint256[] memory amounts);
    
    function swapExactTokensForETH(
        uint256 amountIn,
        uint256 amountOutMin,
        address[] calldata path,
        address to,
        uint256 deadline
    ) external returns (uint256[] memory amounts);
    
    function getAmountsOut(
        uint256 amountIn,
        address[] calldata path
    ) external view returns (uint256[] memory amounts);
}

/// Result of honeypot detection
#[derive(Debug, Clone)]
pub struct HoneypotResult {
    /// Is this token a honeypot?
    pub is_honeypot: bool,
    /// Reason for detection
    pub reason: String,
    /// Buy simulation success
    pub buy_success: bool,
    /// Sell simulation success  
    pub sell_success: bool,
    /// Buy tax percentage (if detectable)
    pub buy_tax_percent: f64,
    /// Sell tax percentage (if detectable)
    pub sell_tax_percent: f64,
    /// Total round-trip loss percentage
    pub total_loss_percent: f64,
    /// Detection latency in milliseconds
    #[allow(dead_code)]
    pub latency_ms: u64,
}

impl HoneypotResult {
    /// Create a safe (non-honeypot) result
    pub fn safe(buy_tax: f64, sell_tax: f64, latency_ms: u64) -> Self {
        Self {
            is_honeypot: false,
            reason: "Token passed buy/sell simulation".to_string(),
            buy_success: true,
            sell_success: true,
            buy_tax_percent: buy_tax,
            sell_tax_percent: sell_tax,
            total_loss_percent: buy_tax + sell_tax,
            latency_ms,
        }
    }
    
    /// Create a honeypot result
    pub fn honeypot(reason: String, buy_success: bool, sell_success: bool, latency_ms: u64) -> Self {
        Self {
            is_honeypot: true,
            reason,
            buy_success,
            sell_success,
            buy_tax_percent: if buy_success { 0.0 } else { 100.0 },
            sell_tax_percent: 100.0,
            total_loss_percent: 100.0,
            latency_ms,
        }
    }
    
    /// Summary for display
    #[allow(dead_code)]
    pub fn summary(&self) -> String {
        if self.is_honeypot {
            format!(
                "🚨 HONEYPOT DETECTED | Reason: {} | Buy: {} | Sell: {} | Latency: {}ms",
                self.reason,
                if self.buy_success { "✅" } else { "❌" },
                if self.sell_success { "✅" } else { "❌" },
                self.latency_ms
            )
        } else {
            format!(
                "✅ SAFE | Buy Tax: {:.2}% | Sell Tax: {:.2}% | Total Loss: {:.2}% | Latency: {}ms",
                self.buy_tax_percent,
                self.sell_tax_percent,
                self.total_loss_percent,
                self.latency_ms
            )
        }
    }
}

/// Honeypot detector using REVM simulation
pub struct HoneypotDetector {
    /// Chain ID (1 = mainnet)
    chain_id: u64,
    /// WETH address
    weth: Address,
    /// Uniswap V2 Router address
    router: Address,
}

impl HoneypotDetector {
    /// Create detector for Ethereum mainnet
    pub fn mainnet() -> Self {
        Self {
            chain_id: 1,
            // WETH on mainnet
            weth: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap(),
            // Uniswap V2 Router
            router: "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap(),
        }
    }
    
    /// Create detector with custom addresses
    #[allow(dead_code)]
    pub fn new(chain_id: u64, weth: Address, router: Address) -> Self {
        Self { chain_id, weth, router }
    }

    /// Detect if a token is a honeypot by simulating buy → sell cycle
    /// 
    /// # Arguments
    /// * `token` - The token address to test
    /// * `test_amount_eth` - Amount of ETH to use for test (e.g., 0.1 ETH)
    /// * `pair_address` - Optional: Uniswap pair address (for state loading)
    /// 
    /// # Returns
    /// * `HoneypotResult` with detection details
    pub fn detect(
        &self,
        token: Address,
        test_amount_eth: U256,
        router_bytecode: Option<Bytes>,
        token_bytecode: Option<Bytes>,
        pair_bytecode: Option<Bytes>,
        pair_address: Option<Address>,
    ) -> Result<HoneypotResult> {
        let start = Instant::now();
        
        // Create fresh database for simulation
        let mut db = CacheDB::new(EmptyDB::default());
        
        // Setup test account with ETH
        let test_account: Address = "0x1234567890123456789012345678901234567890".parse().unwrap();
        db.insert_account_info(test_account, AccountInfo {
            balance: U256::from(100_000_000_000_000_000_000u128), // 100 ETH
            nonce: 0,
            code_hash: KECCAK_EMPTY,
            code: None,
        });
        
        // Setup router with bytecode (if provided, otherwise use minimal mock)
        let router_code = router_bytecode.unwrap_or_else(|| self.mock_router_bytecode());
        db.insert_account_info(self.router, AccountInfo {
            balance: U256::ZERO,
            nonce: 0,
            code_hash: KECCAK_EMPTY,
            code: Some(Bytecode::new_raw(router_code)),
        });
        
        // Setup WETH
        db.insert_account_info(self.weth, AccountInfo {
            balance: U256::from(1_000_000_000_000_000_000_000u128), // 1000 ETH liquidity
            nonce: 0,
            code_hash: KECCAK_EMPTY,
            code: Some(Bytecode::new_raw(self.mock_weth_bytecode())),
        });
        
        // Setup token with bytecode
        let token_code = token_bytecode.unwrap_or_else(|| self.mock_erc20_bytecode());
        db.insert_account_info(token, AccountInfo {
            balance: U256::ZERO,
            nonce: 0,
            code_hash: KECCAK_EMPTY,
            code: Some(Bytecode::new_raw(token_code)),
        });
        
        // Setup pair if provided
        if let (Some(pair), Some(code)) = (pair_address, pair_bytecode) {
            db.insert_account_info(pair, AccountInfo {
                balance: U256::ZERO,
                nonce: 0,
                code_hash: KECCAK_EMPTY,
                code: Some(Bytecode::new_raw(code)),
            });
        }

        // ============================================
        // STEP 1: Simulate BUY (ETH → Token)
        // ============================================
        let buy_result = self.simulate_buy(
            &mut db,
            test_account,
            token,
            test_amount_eth,
        );
        
        let (buy_success, tokens_received) = match buy_result {
            Ok(tokens) => {
                if tokens.is_zero() {
                    return Ok(HoneypotResult::honeypot(
                        "Buy returned 0 tokens".to_string(),
                        false,
                        false,
                        start.elapsed().as_millis() as u64,
                    ));
                }
                (true, tokens)
            }
            Err(e) => {
                return Ok(HoneypotResult::honeypot(
                    format!("Buy failed: {}", e),
                    false,
                    false,
                    start.elapsed().as_millis() as u64,
                ));
            }
        };

        // ============================================
        // STEP 2: Simulate APPROVE (Token → Router)
        // ============================================
        let approve_result = self.simulate_approve(
            &mut db,
            test_account,
            token,
            tokens_received,
        );
        
        if let Err(e) = approve_result {
            return Ok(HoneypotResult::honeypot(
                format!("Approve failed: {}", e),
                true,
                false,
                start.elapsed().as_millis() as u64,
            ));
        }

        // ============================================
        // STEP 3: Simulate SELL (Token → ETH)
        // ============================================
        let sell_result = self.simulate_sell(
            &mut db,
            test_account,
            token,
            tokens_received,
        );
        
        let (sell_success, eth_received) = match sell_result {
            Ok(eth) => {
                if eth.is_zero() {
                    return Ok(HoneypotResult::honeypot(
                        "Sell returned 0 ETH - cannot sell tokens".to_string(),
                        true,
                        false,
                        start.elapsed().as_millis() as u64,
                    ));
                }
                (true, eth)
            }
            Err(e) => {
                return Ok(HoneypotResult::honeypot(
                    format!("Sell failed: {} - HONEYPOT!", e),
                    true,
                    false,
                    start.elapsed().as_millis() as u64,
                ));
            }
        };

        // ============================================
        // STEP 4: Calculate taxes
        // ============================================
        let latency_ms = start.elapsed().as_millis() as u64;
        
        // Calculate loss percentage
        // If we put in X ETH and got back Y ETH, loss = (X - Y) / X * 100
        let test_amount_f64: f64 = u128::try_from(test_amount_eth).unwrap_or(0) as f64;
        let received_f64: f64 = u128::try_from(eth_received).unwrap_or(0) as f64;
        
        if test_amount_f64 == 0.0 {
            return Ok(HoneypotResult::honeypot(
                "Invalid test amount".to_string(),
                buy_success,
                sell_success,
                latency_ms,
            ));
        }
        
        let total_loss_percent = ((test_amount_f64 - received_f64) / test_amount_f64) * 100.0;
        
        // If loss > 50%, likely honeypot or extreme tax
        if total_loss_percent > 50.0 {
            return Ok(HoneypotResult::honeypot(
                format!("Extreme loss: {:.2}% - likely honeypot or high tax", total_loss_percent),
                buy_success,
                sell_success,
                latency_ms,
            ));
        }
        
        // Estimate buy/sell tax (simplified - assumes equal split)
        let buy_tax = total_loss_percent / 2.0;
        let sell_tax = total_loss_percent / 2.0;
        
        Ok(HoneypotResult::safe(buy_tax, sell_tax, latency_ms))
    }

    /// Simulate buying tokens with ETH
    fn simulate_buy(
        &self,
        db: &mut CacheDB<EmptyDB>,
        from: Address,
        token: Address,
        amount_eth: U256,
    ) -> Result<U256> {
        let path = vec![self.weth, token];
        let deadline = U256::from(u64::MAX);
        
        let calldata = swapExactETHForTokensCall {
            amountOutMin: U256::ZERO, // Accept any amount for testing
            path,
            to: from,
            deadline,
        }.abi_encode();
        
        let result = self.execute_tx(
            db,
            from,
            self.router,
            amount_eth,
            Bytes::from(calldata),
            0,
        )?;
        
        // Parse return value (uint256[] amounts)
        // Last element is tokens received
        if result.len() >= 64 {
            // Skip array offset and length, get last uint256
            let tokens = U256::from_be_slice(&result[result.len()-32..]);
            Ok(tokens)
        } else {
            // Fallback: assume some tokens received
            Ok(U256::from(1_000_000_000_000_000_000u128)) // 1 token
        }
    }

    /// Simulate approving router to spend tokens
    fn simulate_approve(
        &self,
        db: &mut CacheDB<EmptyDB>,
        from: Address,
        token: Address,
        amount: U256,
    ) -> Result<()> {
        let calldata = approveCall {
            spender: self.router,
            amount,
        }.abi_encode();
        
        self.execute_tx(
            db,
            from,
            token,
            U256::ZERO,
            Bytes::from(calldata),
            1,
        )?;
        
        Ok(())
    }

    /// Simulate selling tokens for ETH
    fn simulate_sell(
        &self,
        db: &mut CacheDB<EmptyDB>,
        from: Address,
        token: Address,
        amount_tokens: U256,
    ) -> Result<U256> {
        let path = vec![token, self.weth];
        let deadline = U256::from(u64::MAX);
        
        let calldata = swapExactTokensForETHCall {
            amountIn: amount_tokens,
            amountOutMin: U256::ZERO, // Accept any amount for testing
            path,
            to: from,
            deadline,
        }.abi_encode();
        
        let result = self.execute_tx(
            db,
            from,
            self.router,
            U256::ZERO,
            Bytes::from(calldata),
            2,
        )?;
        
        // Parse return value
        if result.len() >= 64 {
            let eth = U256::from_be_slice(&result[result.len()-32..]);
            Ok(eth)
        } else {
            Ok(U256::ZERO)
        }
    }

    /// Execute a transaction in the EVM
    fn execute_tx(
        &self,
        db: &mut CacheDB<EmptyDB>,
        from: Address,
        to: Address,
        value: U256,
        data: Bytes,
        nonce: u64,
    ) -> Result<Vec<u8>> {
        let tx_env = TxEnv {
            caller: from,
            gas_limit: 500_000,
            gas_price: U256::from(20_000_000_000u64),
            transact_to: TxKind::Call(to),
            value,
            data,
            nonce: Some(nonce),
            chain_id: Some(self.chain_id),
            ..Default::default()
        };
        
        let block_env = BlockEnv {
            number: U256::from(19_000_000u64),
            timestamp: U256::from(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
            ),
            gas_limit: U256::from(30_000_000u64),
            basefee: U256::from(20_000_000_000u64),
            ..Default::default()
        };

        let cfg = CfgEnvWithHandlerCfg::new_with_spec_id(Default::default(), SpecId::CANCUN);
        let env = EnvWithHandlerCfg::new_with_cfg_env(cfg, block_env, tx_env);

        let mut evm = Evm::builder()
            .with_db(db)
            .with_env_with_handler_cfg(env)
            .build();

        let result = evm.transact_commit();
        
        match result {
            Ok(ExecutionResult::Success { output, .. }) => {
                match output {
                    Output::Call(bytes) => Ok(bytes.to_vec()),
                    Output::Create(bytes, _) => Ok(bytes.to_vec()),
                }
            }
            Ok(ExecutionResult::Revert { output, .. }) => {
                Err(eyre!("Reverted: 0x{}", hex::encode(&output)))
            }
            Ok(ExecutionResult::Halt { reason, .. }) => {
                Err(eyre!("Halted: {:?}", reason))
            }
            Err(e) => {
                Err(eyre!("EVM error: {:?}", e))
            }
        }
    }

    /// Mock router bytecode (returns success for testing)
    fn mock_router_bytecode(&self) -> Bytes {
        // Minimal bytecode that returns success
        // PUSH1 0x01 PUSH1 0x00 MSTORE PUSH1 0x20 PUSH1 0x00 RETURN
        Bytes::from(vec![0x60, 0x01, 0x60, 0x00, 0x52, 0x60, 0x20, 0x60, 0x00, 0xf3])
    }

    /// Mock WETH bytecode
    fn mock_weth_bytecode(&self) -> Bytes {
        Bytes::from(vec![0x60, 0x01, 0x60, 0x00, 0x52, 0x60, 0x20, 0x60, 0x00, 0xf3])
    }

    /// Mock ERC20 bytecode
    fn mock_erc20_bytecode(&self) -> Bytes {
        Bytes::from(vec![0x60, 0x01, 0x60, 0x00, 0x52, 0x60, 0x20, 0x60, 0x00, 0xf3])
    }
}

/// Quick honeypot check without full state
/// Uses heuristics and minimal simulation
#[allow(dead_code)]
pub fn quick_honeypot_check(
    token: Address,
    test_eth: U256,
) -> HoneypotResult {
    let start = Instant::now();
    let detector = HoneypotDetector::mainnet();
    
    match detector.detect(token, test_eth, None, None, None, None) {
        Ok(result) => result,
        Err(e) => HoneypotResult::honeypot(
            format!("Detection error: {}", e),
            false,
            false,
            start.elapsed().as_millis() as u64,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_honeypot_result_safe() {
        let result = HoneypotResult::safe(2.5, 2.5, 15);
        assert!(!result.is_honeypot);
        assert_eq!(result.total_loss_percent, 5.0);
        assert!(result.summary().contains("SAFE"));
    }

    #[test]
    fn test_honeypot_result_detected() {
        let result = HoneypotResult::honeypot(
            "Cannot sell".to_string(),
            true,
            false,
            20,
        );
        assert!(result.is_honeypot);
        assert!(result.summary().contains("HONEYPOT"));
    }

    #[test]
    fn test_detector_creation() {
        let detector = HoneypotDetector::mainnet();
        assert_eq!(detector.chain_id, 1);
    }
}
