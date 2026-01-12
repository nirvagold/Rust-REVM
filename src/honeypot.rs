//! Honeypot Detection Module
//! Simulates Buy → Sell cycle in-memory to detect honeypot tokens
//!
//! PERS Algorithm (Pre-Execution Risk Scoring):
//! 1. Generate RANDOM caller address (not deployer) to avoid whitelist bypass
//! 2. Simulate Buy (ETH → Token)
//! 3. Simulate Approve (Token → Router)
//! 4. Simulate Sell (Token → ETH) - REVERT = HONEYPOT!
//! 5. Scan bytecode for Access Control functions (blacklist, setBots)
//!
//! If sell reverts → is_honeypot = true, risk_score = 100
//! If blacklist functions detected → risk_score += 50

use alloy_primitives::{Address, Bytes, FixedBytes, U256};
use alloy_sol_types::{sol, SolCall};
use eyre::{eyre, Result};
use rand::Rng;
use revm::{
    db::{CacheDB, EmptyDB},
    primitives::{
        AccountInfo, BlockEnv, Bytecode, CfgEnvWithHandlerCfg, EnvWithHandlerCfg, ExecutionResult,
        Output, SpecId, TxEnv, TxKind, KECCAK_EMPTY,
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
    /// Sell reverted (critical honeypot indicator)
    pub sell_reverted: bool,
    /// Buy tax percentage (if detectable)
    pub buy_tax_percent: f64,
    /// Sell tax percentage (if detectable)
    pub sell_tax_percent: f64,
    /// Total round-trip loss percentage
    pub total_loss_percent: f64,
    /// Access control penalty (blacklist/setBots detected)
    pub access_control_penalty: u8,
    /// Risk factors detected
    #[allow(dead_code)]
    pub risk_factors: Vec<String>,
    /// Detection latency in milliseconds
    #[allow(dead_code)]
    pub latency_ms: u64,
}

impl HoneypotResult {
    /// Create a safe (non-honeypot) result
    pub fn safe(
        buy_tax: f64,
        sell_tax: f64,
        access_penalty: u8,
        risk_factors: Vec<String>,
        latency_ms: u64,
    ) -> Self {
        Self {
            is_honeypot: false,
            reason: "Token passed buy/sell simulation".to_string(),
            buy_success: true,
            sell_success: true,
            sell_reverted: false,
            buy_tax_percent: buy_tax,
            sell_tax_percent: sell_tax,
            total_loss_percent: buy_tax + sell_tax,
            access_control_penalty: access_penalty,
            risk_factors,
            latency_ms,
        }
    }

    /// Create a honeypot result
    pub fn honeypot(
        reason: String,
        buy_success: bool,
        sell_success: bool,
        sell_reverted: bool,
        access_penalty: u8,
        risk_factors: Vec<String>,
        latency_ms: u64,
    ) -> Self {
        Self {
            is_honeypot: true,
            reason,
            buy_success,
            sell_success,
            sell_reverted,
            buy_tax_percent: if buy_success { 0.0 } else { 100.0 },
            sell_tax_percent: 100.0,
            total_loss_percent: 100.0,
            access_control_penalty: access_penalty,
            risk_factors,
            latency_ms,
        }
    }

    /// Summary for display
    #[allow(dead_code)]
    pub fn summary(&self) -> String {
        if self.is_honeypot {
            format!(
                "🚨 HONEYPOT DETECTED | Reason: {} | Buy: {} | Sell: {} | Reverted: {} | Latency: {}ms",
                self.reason,
                if self.buy_success { "✅" } else { "❌" },
                if self.sell_success { "✅" } else { "❌" },
                if self.sell_reverted { "⛔" } else { "✅" },
                self.latency_ms
            )
        } else {
            format!(
                "✅ SAFE | Buy Tax: {:.2}% | Sell Tax: {:.2}% | Total Loss: {:.2}% | AC Penalty: {} | Latency: {}ms",
                self.buy_tax_percent,
                self.sell_tax_percent,
                self.total_loss_percent,
                self.access_control_penalty,
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

/// Result of sell simulation with revert detection
enum SimSellResult {
    Success(U256),
    Reverted(String),
}

impl HoneypotDetector {
    /// Create detector for Ethereum mainnet
    pub fn mainnet() -> Self {
        Self {
            chain_id: 1,
            // WETH on mainnet
            weth: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
                .parse()
                .unwrap(),
            // Uniswap V2 Router
            router: "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D"
                .parse()
                .unwrap(),
        }
    }

    /// Create detector with custom addresses
    #[allow(dead_code)]
    pub fn new(chain_id: u64, weth: Address, router: Address) -> Self {
        Self {
            chain_id,
            weth,
            router,
        }
    }

    /// Detect if a token is a honeypot by simulating buy → sell cycle
    ///
    /// PERS Algorithm v2:
    /// 1. Generate RANDOM caller address (prevents whitelist bypass)
    /// 2. Scan bytecode for access control functions
    /// 3. Simulate Buy → Approve → Sell cycle
    /// 4. If SELL REVERTS → is_honeypot = true, risk_score = 100
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
        let mut risk_factors: Vec<String> = Vec::new();

        // Create fresh database for simulation
        let mut db = CacheDB::new(EmptyDB::default());

        // ============================================
        // STEP 0: Generate RANDOM caller address
        // This prevents honeypots from whitelisting deployer/known addresses
        // ============================================
        let test_account = Self::generate_random_address();

        db.insert_account_info(
            test_account,
            AccountInfo {
                balance: U256::from(100_000_000_000_000_000_000u128), // 100 ETH
                nonce: 0,
                code_hash: KECCAK_EMPTY,
                code: None,
            },
        );

        // Setup router with bytecode (if provided, otherwise use minimal mock)
        let router_code = router_bytecode.unwrap_or_else(|| self.mock_router_bytecode());
        db.insert_account_info(
            self.router,
            AccountInfo {
                balance: U256::ZERO,
                nonce: 0,
                code_hash: KECCAK_EMPTY,
                code: Some(Bytecode::new_raw(router_code)),
            },
        );

        // Setup WETH
        db.insert_account_info(
            self.weth,
            AccountInfo {
                balance: U256::from(1_000_000_000_000_000_000_000u128), // 1000 ETH liquidity
                nonce: 0,
                code_hash: KECCAK_EMPTY,
                code: Some(Bytecode::new_raw(self.mock_weth_bytecode())),
            },
        );

        // Setup token with bytecode
        let token_code = token_bytecode
            .clone()
            .unwrap_or_else(|| self.mock_erc20_bytecode());
        db.insert_account_info(
            token,
            AccountInfo {
                balance: U256::ZERO,
                nonce: 0,
                code_hash: KECCAK_EMPTY,
                code: Some(Bytecode::new_raw(token_code.clone())),
            },
        );

        // Setup pair if provided
        if let (Some(pair), Some(code)) = (pair_address, pair_bytecode) {
            db.insert_account_info(
                pair,
                AccountInfo {
                    balance: U256::ZERO,
                    nonce: 0,
                    code_hash: KECCAK_EMPTY,
                    code: Some(Bytecode::new_raw(code)),
                },
            );
        }

        // ============================================
        // STEP 1: Scan bytecode for Access Control functions
        // Detect: setBots, blacklistAddress, addBot, isBot, etc.
        // ============================================
        let access_control_penalty =
            self.scan_access_control_functions(&token_code, &mut risk_factors);

        // ============================================
        // STEP 2: Simulate BUY (ETH → Token)
        // ============================================
        let buy_result = self.simulate_buy(&mut db, test_account, token, test_amount_eth);

        let (buy_success, tokens_received) = match buy_result {
            Ok(tokens) => {
                if tokens.is_zero() {
                    return Ok(HoneypotResult::honeypot(
                        "Buy returned 0 tokens".to_string(),
                        false,
                        false,
                        false,
                        access_control_penalty,
                        risk_factors,
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
                    false,
                    access_control_penalty,
                    risk_factors,
                    start.elapsed().as_millis() as u64,
                ));
            }
        };

        // ============================================
        // STEP 3: Simulate APPROVE (Token → Router)
        // ============================================
        let approve_result = self.simulate_approve(&mut db, test_account, token, tokens_received);

        if let Err(e) = approve_result {
            return Ok(HoneypotResult::honeypot(
                format!("Approve failed: {}", e),
                true,
                false,
                false,
                access_control_penalty,
                risk_factors,
                start.elapsed().as_millis() as u64,
            ));
        }

        // ============================================
        // STEP 4: Simulate SELL (Token → ETH)
        // CRITICAL: If this REVERTS → HONEYPOT with risk_score = 100
        // ============================================
        let sell_result =
            self.simulate_sell_with_revert_detection(&mut db, test_account, token, tokens_received);

        let (sell_success, sell_reverted, eth_received) = match sell_result {
            Ok(SimSellResult::Success(eth)) => {
                if eth.is_zero() {
                    // Sell returned 0 ETH - honeypot!
                    return Ok(HoneypotResult::honeypot(
                        "Sell returned 0 ETH - cannot sell tokens".to_string(),
                        true,
                        false,
                        false,
                        access_control_penalty,
                        risk_factors,
                        start.elapsed().as_millis() as u64,
                    ));
                }
                (true, false, eth)
            }
            Ok(SimSellResult::Reverted(reason)) => {
                // ⛔ SELL REVERTED = HONEYPOT! risk_score = 100
                risk_factors.push(format!("SELL REVERTED: {}", reason));
                return Ok(HoneypotResult::honeypot(
                    format!("⛔ SELL REVERTED: {} - CONFIRMED HONEYPOT!", reason),
                    true,
                    false,
                    true, // sell_reverted = true
                    access_control_penalty,
                    risk_factors,
                    start.elapsed().as_millis() as u64,
                ));
            }
            Err(e) => {
                return Ok(HoneypotResult::honeypot(
                    format!("Sell failed: {} - HONEYPOT!", e),
                    true,
                    false,
                    true,
                    access_control_penalty,
                    risk_factors,
                    start.elapsed().as_millis() as u64,
                ));
            }
        };

        // ============================================
        // STEP 5: Calculate taxes
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
                sell_reverted,
                access_control_penalty,
                risk_factors,
                latency_ms,
            ));
        }

        let total_loss_percent = ((test_amount_f64 - received_f64) / test_amount_f64) * 100.0;

        // If loss > 50%, likely honeypot or extreme tax
        if total_loss_percent > 50.0 {
            risk_factors.push(format!("Extreme loss: {:.2}%", total_loss_percent));
            return Ok(HoneypotResult::honeypot(
                format!(
                    "Extreme loss: {:.2}% - likely honeypot or high tax",
                    total_loss_percent
                ),
                buy_success,
                sell_success,
                sell_reverted,
                access_control_penalty,
                risk_factors,
                latency_ms,
            ));
        }

        // Estimate buy/sell tax (simplified - assumes equal split)
        let buy_tax = total_loss_percent / 2.0;
        let sell_tax = total_loss_percent / 2.0;

        Ok(HoneypotResult::safe(
            buy_tax,
            sell_tax,
            access_control_penalty,
            risk_factors,
            latency_ms,
        ))
    }

    /// Generate a random Ethereum address for simulation
    /// This prevents honeypots from whitelisting known addresses
    fn generate_random_address() -> Address {
        let mut rng = rand::thread_rng();
        let mut bytes = [0u8; 20];
        rng.fill(&mut bytes);
        Address::from(FixedBytes::<20>::from(bytes))
    }

    /// Scan bytecode for access control functions that could be used for blacklisting
    /// Returns penalty score (0 or 50)
    fn scan_access_control_functions(
        &self,
        bytecode: &Bytes,
        risk_factors: &mut Vec<String>,
    ) -> u8 {
        let code_hex = hex::encode(bytecode.as_ref());
        let mut penalty: u8 = 0;

        // Function selectors for dangerous access control functions
        // These are keccak256 hashes of function signatures (first 4 bytes)
        let dangerous_selectors = [
            // setBots(address[],bool) - common honeypot function
            ("974d396d", "setBots"),
            // setBot(address,bool)
            ("3d18678e", "setBot"),
            // blacklistAddress(address)
            ("e4997dc5", "blacklistAddress"),
            // addToBlacklist(address)
            ("44337ea1", "addToBlacklist"),
            // isBot(address)
            ("b515566a", "isBot"),
            // setBlacklist(address,bool)
            ("0ecb93c0", "setBlacklist"),
            // addBot(address)
            ("09218e91", "addBot"),
            // delBot(address)
            ("363bf964", "delBot"),
            // setTradingEnabled(bool) - can disable trading
            ("8a8c523c", "setTradingEnabled"),
            // enableTrading()
            ("8da5cb5b", "enableTrading"),
            // setMaxTxAmount - can limit sells
            ("ec28438a", "setMaxTxAmount"),
            // setMaxWalletSize - can limit holdings
            ("f1d5f517", "setMaxWalletSize"),
        ];

        for (selector, name) in dangerous_selectors.iter() {
            if code_hex.contains(selector) {
                risk_factors.push(format!("⚠️ Access Control: {} detected", name));
                penalty = 50; // +50 penalty for any access control function
            }
        }

        // Also check for common blacklist storage patterns
        // mapping(address => bool) bots/blacklist
        if code_hex.contains("626f7473") || // "bots" in hex
           code_hex.contains("626c61636b6c697374")
        {
            // "blacklist" in hex
            if penalty == 0 {
                risk_factors.push("⚠️ Blacklist storage pattern detected".to_string());
                penalty = 50;
            }
        }

        penalty
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
        }
        .abi_encode();

        let result =
            self.execute_tx(db, from, self.router, amount_eth, Bytes::from(calldata), 0)?;

        // Parse return value (uint256[] amounts)
        // Last element is tokens received
        if result.len() >= 64 {
            // Skip array offset and length, get last uint256
            let tokens = U256::from_be_slice(&result[result.len() - 32..]);
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
        }
        .abi_encode();

        self.execute_tx(db, from, token, U256::ZERO, Bytes::from(calldata), 1)?;

        Ok(())
    }

    /// Simulate selling tokens for ETH
    #[allow(dead_code)]
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
        }
        .abi_encode();

        let result =
            self.execute_tx(db, from, self.router, U256::ZERO, Bytes::from(calldata), 2)?;

        // Parse return value
        if result.len() >= 64 {
            let eth = U256::from_be_slice(&result[result.len() - 32..]);
            Ok(eth)
        } else {
            Ok(U256::ZERO)
        }
    }

    /// Simulate selling tokens with explicit revert detection
    /// This is the CRITICAL function for honeypot detection
    fn simulate_sell_with_revert_detection(
        &self,
        db: &mut CacheDB<EmptyDB>,
        from: Address,
        token: Address,
        amount_tokens: U256,
    ) -> Result<SimSellResult> {
        let path = vec![token, self.weth];
        let deadline = U256::from(u64::MAX);

        let calldata = swapExactTokensForETHCall {
            amountIn: amount_tokens,
            amountOutMin: U256::ZERO,
            path,
            to: from,
            deadline,
        }
        .abi_encode();

        let tx_env = TxEnv {
            caller: from,
            gas_limit: 500_000,
            gas_price: U256::from(20_000_000_000u64),
            transact_to: TxKind::Call(self.router),
            value: U256::ZERO,
            data: Bytes::from(calldata),
            nonce: Some(2),
            chain_id: Some(self.chain_id),
            ..Default::default()
        };

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

        let cfg = CfgEnvWithHandlerCfg::new_with_spec_id(Default::default(), SpecId::CANCUN);
        let env = EnvWithHandlerCfg::new_with_cfg_env(cfg, block_env, tx_env);

        let mut evm = Evm::builder()
            .with_db(db)
            .with_env_with_handler_cfg(env)
            .build();

        let result = evm.transact_commit();

        match result {
            Ok(ExecutionResult::Success { output, .. }) => match output {
                Output::Call(bytes) => {
                    if bytes.len() >= 64 {
                        let eth = U256::from_be_slice(&bytes[bytes.len() - 32..]);
                        Ok(SimSellResult::Success(eth))
                    } else {
                        Ok(SimSellResult::Success(U256::ZERO))
                    }
                }
                Output::Create(_, _) => Ok(SimSellResult::Success(U256::ZERO)),
            },
            Ok(ExecutionResult::Revert { output, .. }) => {
                // ⛔ REVERT DETECTED - This is a HONEYPOT!
                let reason = Self::decode_revert_reason(&output);
                Ok(SimSellResult::Reverted(reason))
            }
            Ok(ExecutionResult::Halt { reason, .. }) => {
                Ok(SimSellResult::Reverted(format!("Halted: {:?}", reason)))
            }
            Err(e) => Err(eyre!("EVM error: {:?}", e)),
        }
    }

    /// Decode revert reason from output bytes
    fn decode_revert_reason(output: &Bytes) -> String {
        // Try to decode Error(string) selector: 0x08c379a0
        if output.len() >= 68 && output[0..4] == [0x08, 0xc3, 0x79, 0xa0] {
            // Skip selector (4) + offset (32) + length position
            let len_start = 36;
            if output.len() > len_start + 32 {
                let len = U256::from_be_slice(&output[len_start..len_start + 32]);
                let len_usize: usize = len.try_into().unwrap_or(0);
                let str_start = len_start + 32;
                if output.len() >= str_start + len_usize {
                    if let Ok(s) =
                        String::from_utf8(output[str_start..str_start + len_usize].to_vec())
                    {
                        return s;
                    }
                }
            }
        }

        // Common revert reasons in honeypots
        let hex_output = hex::encode(output.as_ref());
        if hex_output.contains("626f74") {
            // "bot"
            return "Bot detected / Blacklisted".to_string();
        }
        if hex_output.contains("74726164696e67") {
            // "trading"
            return "Trading not enabled".to_string();
        }
        if hex_output.contains("7472616e73666572") {
            // "transfer"
            return "Transfer blocked".to_string();
        }

        format!("Revert: 0x{}", hex::encode(&output[..output.len().min(64)]))
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
                    .unwrap_or(0),
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
            Ok(ExecutionResult::Success { output, .. }) => match output {
                Output::Call(bytes) => Ok(bytes.to_vec()),
                Output::Create(bytes, _) => Ok(bytes.to_vec()),
            },
            Ok(ExecutionResult::Revert { output, .. }) => {
                Err(eyre!("Reverted: 0x{}", hex::encode(&output)))
            }
            Ok(ExecutionResult::Halt { reason, .. }) => Err(eyre!("Halted: {:?}", reason)),
            Err(e) => Err(eyre!("EVM error: {:?}", e)),
        }
    }

    /// Mock router bytecode (returns success for testing)
    fn mock_router_bytecode(&self) -> Bytes {
        // Minimal bytecode that returns success
        // PUSH1 0x01 PUSH1 0x00 MSTORE PUSH1 0x20 PUSH1 0x00 RETURN
        Bytes::from(vec![
            0x60, 0x01, 0x60, 0x00, 0x52, 0x60, 0x20, 0x60, 0x00, 0xf3,
        ])
    }

    /// Mock WETH bytecode
    fn mock_weth_bytecode(&self) -> Bytes {
        Bytes::from(vec![
            0x60, 0x01, 0x60, 0x00, 0x52, 0x60, 0x20, 0x60, 0x00, 0xf3,
        ])
    }

    /// Mock ERC20 bytecode
    fn mock_erc20_bytecode(&self) -> Bytes {
        Bytes::from(vec![
            0x60, 0x01, 0x60, 0x00, 0x52, 0x60, 0x20, 0x60, 0x00, 0xf3,
        ])
    }
}

/// Quick honeypot check without full state
/// Uses heuristics and minimal simulation
#[allow(dead_code)]
pub fn quick_honeypot_check(token: Address, test_eth: U256) -> HoneypotResult {
    let start = Instant::now();
    let detector = HoneypotDetector::mainnet();

    match detector.detect(token, test_eth, None, None, None, None) {
        Ok(result) => result,
        Err(e) => HoneypotResult::honeypot(
            format!("Detection error: {}", e),
            false,
            false,
            false,
            0,
            vec![format!("Error: {}", e)],
            start.elapsed().as_millis() as u64,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_honeypot_result_safe() {
        let result = HoneypotResult::safe(2.5, 2.5, 0, vec![], 15);
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
            true,
            50,
            vec!["setBots detected".to_string()],
            20,
        );
        assert!(result.is_honeypot);
        assert!(result.sell_reverted);
        assert_eq!(result.access_control_penalty, 50);
        assert!(result.summary().contains("HONEYPOT"));
    }

    #[test]
    fn test_detector_creation() {
        let detector = HoneypotDetector::mainnet();
        assert_eq!(detector.chain_id, 1);
    }

    #[test]
    fn test_random_address_generation() {
        let addr1 = HoneypotDetector::generate_random_address();
        let addr2 = HoneypotDetector::generate_random_address();
        // Should generate different addresses
        assert_ne!(addr1, addr2);
    }

    #[test]
    fn test_access_control_scan() {
        let detector = HoneypotDetector::mainnet();
        let mut risk_factors = Vec::new();

        // Bytecode containing setBots selector (974d396d)
        let malicious_bytecode =
            Bytes::from(hex::decode("608060405234801561001057600080fd5b50974d396d").unwrap());
        let penalty =
            detector.scan_access_control_functions(&malicious_bytecode, &mut risk_factors);

        assert_eq!(penalty, 50);
        assert!(!risk_factors.is_empty());
    }
}
