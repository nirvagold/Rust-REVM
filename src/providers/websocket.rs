//! Alchemy WebSocket Subscriptions Module
//!
//! Real-time blockchain event streaming for sniper bot optimization.
//! 10x faster than polling for detecting new tokens and transactions.
//!
//! Supported Subscriptions:
//! 1. newHeads - New blocks (all EVM chains)
//! 2. logs - Event logs with topic filters (Transfer, Swap, PairCreated)
//! 3. alchemy_pendingTransactions - Pending tx (ETH, Polygon only)
//! 4. alchemy_minedTransactions - Mined tx with filters
//!
//! Alchemy Documentation Reference:
//! - Subscription API: https://alchemy.com/docs/reference/subscription-api.mdx
//! - Best Practices: https://alchemy.com/docs/reference/best-practices-for-using-websockets-in-web3.mdx
//! - newHeads: https://alchemy.com/docs/reference/newheads.mdx
//! - logs: https://alchemy.com/docs/reference/logs.mdx
//! - alchemy_pendingTransactions: https://alchemy.com/docs/reference/alchemy-pendingtransactions.mdx
//!
//! Best Practices (from Alchemy docs):
//! - Use HTTPS for JSON-RPC, WebSockets ONLY for subscriptions
//! - Implement reconnection logic with exponential backoff
//! - Handle connection drops gracefully
//! - Use filters to reduce data volume

use eyre::{eyre, Result};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

use crate::utils::constants::{
    get_alchemy_subdomain, CHAIN_ID_ETHEREUM, CHAIN_ID_POLYGON,
};

// ============================================
// WEBSOCKET CONSTANTS
// ============================================

/// Reconnection base delay (milliseconds)
const WS_RECONNECT_BASE_MS: u64 = 1000;

/// Maximum reconnection delay (milliseconds)
const WS_RECONNECT_MAX_MS: u64 = 30000;

/// Ping interval (seconds) - reserved for future heartbeat implementation
#[allow(dead_code)]
const WS_PING_INTERVAL_SECS: u64 = 30;

/// Maximum reconnection attempts before giving up
const WS_MAX_RECONNECT_ATTEMPTS: u32 = 10;

// ============================================
// EVENT TYPES
// ============================================

/// New block header from newHeads subscription
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockHeader {
    pub number: String,
    pub hash: String,
    pub parent_hash: String,
    pub timestamp: String,
    pub gas_used: String,
    pub gas_limit: String,
    pub base_fee_per_gas: Option<String>,
    pub miner: String,
    pub transactions_root: String,
}

/// Log event from logs subscription
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEvent {
    pub address: String,
    pub topics: Vec<String>,
    pub data: String,
    pub block_number: String,
    pub block_hash: String,
    pub transaction_hash: String,
    pub transaction_index: String,
    pub log_index: String,
    pub removed: bool,
}

/// Pending transaction from alchemy_pendingTransactions
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingTransaction {
    pub hash: String,
    pub from: String,
    pub to: Option<String>,
    pub value: String,
    pub gas: String,
    pub gas_price: Option<String>,
    pub max_fee_per_gas: Option<String>,
    pub max_priority_fee_per_gas: Option<String>,
    pub input: String,
    pub nonce: String,
}

/// Mined transaction from alchemy_minedTransactions
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MinedTransaction {
    pub transaction: TransactionInfo,
    pub block_hash: String,
    pub block_number: String,
}

/// Transaction info within mined transaction
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionInfo {
    pub hash: String,
    pub from: String,
    pub to: Option<String>,
    pub value: String,
    pub input: String,
}

/// Unified WebSocket event enum
#[derive(Debug, Clone)]
pub enum WsEvent {
    NewBlock(BlockHeader),
    Log(LogEvent),
    PendingTx(PendingTransaction),
    MinedTx(MinedTransaction),
    Connected,
    Disconnected,
    Error(String),
}

// ============================================
// SUBSCRIPTION TYPES
// ============================================

/// Subscription type for WebSocket
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SubscriptionType {
    NewHeads,
    Logs,
    AlchemyPendingTransactions,
    AlchemyMinedTransactions,
}

/// Log filter for logs subscription
#[derive(Debug, Clone, Serialize, Default)]
pub struct LogFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topics: Option<Vec<Option<String>>>,
}

impl LogFilter {
    /// Create filter for PairCreated events (Uniswap V2 factory)
    pub fn pair_created() -> Self {
        Self {
            address: None,
            // PairCreated(address,address,address,uint256) topic0
            topics: Some(vec![Some(
                "0x0d3648bd0f6ba80134a33ba9275ac585d9d315f0ad8355cddefde31afa28d0e9".to_string(),
            )]),
        }
    }

    /// Create filter for Transfer events
    pub fn transfer() -> Self {
        Self {
            address: None,
            // Transfer(address,address,uint256) topic0
            topics: Some(vec![Some(
                "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef".to_string(),
            )]),
        }
    }

    /// Create filter for Swap events (Uniswap V2)
    pub fn swap() -> Self {
        Self {
            address: None,
            // Swap(address,uint256,uint256,uint256,uint256,address) topic0
            topics: Some(vec![Some(
                "0xd78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822".to_string(),
            )]),
        }
    }

    /// Create filter for specific contract address
    pub fn for_address(address: &str) -> Self {
        Self {
            address: Some(vec![address.to_string()]),
            topics: None,
        }
    }
}

/// Pending transaction filter
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PendingTxFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash_only: Option<bool>,
}

impl PendingTxFilter {
    /// Filter for transactions TO a specific address (e.g., DEX router)
    pub fn to_address(address: &str) -> Self {
        Self {
            from_address: None,
            to_address: Some(address.to_string()),
            hash_only: Some(false),
        }
    }
}

// ============================================
// WEBSOCKET CLIENT
// ============================================

/// Alchemy WebSocket Client for real-time subscriptions
pub struct AlchemyWsClient {
    chain_id: u64,
    api_key: String,
    is_connected: Arc<AtomicBool>,
    subscription_id: Arc<AtomicU64>,
}

impl AlchemyWsClient {
    /// Create new WebSocket client for a chain
    pub fn new(chain_id: u64) -> Result<Self> {
        let api_key = std::env::var("ALCHEMY_API_KEY")
            .map_err(|_| eyre!("ALCHEMY_API_KEY not configured"))?;

        if api_key.is_empty() || api_key == "YOUR_API_KEY" {
            return Err(eyre!("Invalid ALCHEMY_API_KEY"));
        }

        Ok(Self {
            chain_id,
            api_key,
            is_connected: Arc::new(AtomicBool::new(false)),
            subscription_id: Arc::new(AtomicU64::new(1)),
        })
    }

    /// Get WebSocket URL for this chain
    fn ws_url(&self) -> Result<String> {
        let subdomain = get_alchemy_subdomain(self.chain_id)
            .ok_or_else(|| eyre!("Unsupported chain for WebSocket: {}", self.chain_id))?;

        Ok(format!(
            "wss://{}.g.alchemy.com/v2/{}",
            subdomain, self.api_key
        ))
    }

    /// Check if pending transactions are supported (ETH, Polygon only)
    pub fn supports_pending_tx(&self) -> bool {
        matches!(self.chain_id, CHAIN_ID_ETHEREUM | CHAIN_ID_POLYGON)
    }

    /// Subscribe to new blocks (newHeads)
    /// 
    /// Returns a channel receiver for block events
    pub async fn subscribe_new_heads(&self) -> Result<mpsc::Receiver<WsEvent>> {
        let (tx, rx) = mpsc::channel(100);
        let url = self.ws_url()?;
        let is_connected = self.is_connected.clone();
        let sub_id = self.subscription_id.fetch_add(1, Ordering::SeqCst);

        tokio::spawn(async move {
            Self::run_subscription(
                url,
                "eth_subscribe",
                serde_json::json!(["newHeads"]),
                sub_id,
                tx,
                is_connected,
                Self::parse_new_head,
            )
            .await;
        });

        Ok(rx)
    }

    /// Subscribe to log events with filter
    /// 
    /// Use LogFilter::pair_created() for new token detection!
    pub async fn subscribe_logs(&self, filter: LogFilter) -> Result<mpsc::Receiver<WsEvent>> {
        let (tx, rx) = mpsc::channel(100);
        let url = self.ws_url()?;
        let is_connected = self.is_connected.clone();
        let sub_id = self.subscription_id.fetch_add(1, Ordering::SeqCst);

        let params = serde_json::json!(["logs", filter]);

        tokio::spawn(async move {
            Self::run_subscription(
                url,
                "eth_subscribe",
                params,
                sub_id,
                tx,
                is_connected,
                Self::parse_log,
            )
            .await;
        });

        Ok(rx)
    }

    /// Subscribe to pending transactions (ETH, Polygon only)
    /// 
    /// WARNING: High volume! Use filters to reduce data.
    pub async fn subscribe_pending_tx(
        &self,
        filter: Option<PendingTxFilter>,
    ) -> Result<mpsc::Receiver<WsEvent>> {
        if !self.supports_pending_tx() {
            return Err(eyre!(
                "Pending transactions not supported on chain {}. Only ETH and Polygon.",
                self.chain_id
            ));
        }

        let (tx, rx) = mpsc::channel(1000); // Higher buffer for pending tx
        let url = self.ws_url()?;
        let is_connected = self.is_connected.clone();
        let sub_id = self.subscription_id.fetch_add(1, Ordering::SeqCst);

        let params = if let Some(f) = filter {
            serde_json::json!(["alchemy_pendingTransactions", f])
        } else {
            serde_json::json!(["alchemy_pendingTransactions"])
        };

        tokio::spawn(async move {
            Self::run_subscription(
                url,
                "eth_subscribe",
                params,
                sub_id,
                tx,
                is_connected,
                Self::parse_pending_tx,
            )
            .await;
        });

        Ok(rx)
    }

    /// Subscribe to mined transactions
    pub async fn subscribe_mined_tx(&self) -> Result<mpsc::Receiver<WsEvent>> {
        let (tx, rx) = mpsc::channel(100);
        let url = self.ws_url()?;
        let is_connected = self.is_connected.clone();
        let sub_id = self.subscription_id.fetch_add(1, Ordering::SeqCst);

        let params = serde_json::json!(["alchemy_minedTransactions", {
            "includeRemoved": false,
            "hashesOnly": false
        }]);

        tokio::spawn(async move {
            Self::run_subscription(
                url,
                "eth_subscribe",
                params,
                sub_id,
                tx,
                is_connected,
                Self::parse_mined_tx,
            )
            .await;
        });

        Ok(rx)
    }

    /// Internal: Run subscription with reconnection logic
    async fn run_subscription<F>(
        url: String,
        method: &str,
        params: serde_json::Value,
        sub_id: u64,
        tx: mpsc::Sender<WsEvent>,
        is_connected: Arc<AtomicBool>,
        parser: F,
    ) where
        F: Fn(&str) -> Option<WsEvent> + Send + 'static,
    {
        let mut reconnect_attempts = 0;
        let mut reconnect_delay = WS_RECONNECT_BASE_MS;

        loop {
            match connect_async(&url).await {
                Ok((ws_stream, _)) => {
                    info!("🔌 WebSocket connected to Alchemy");
                    is_connected.store(true, Ordering::SeqCst);
                    reconnect_attempts = 0;
                    reconnect_delay = WS_RECONNECT_BASE_MS;

                    let _ = tx.send(WsEvent::Connected).await;

                    let (mut write, mut read) = ws_stream.split();

                    // Send subscription request
                    let subscribe_msg = serde_json::json!({
                        "jsonrpc": "2.0",
                        "method": method,
                        "params": params,
                        "id": sub_id
                    });

                    if let Err(e) = write.send(Message::Text(subscribe_msg.to_string())).await {
                        error!("❌ Failed to send subscription: {}", e);
                        continue;
                    }

                    // Process messages
                    while let Some(msg_result) = read.next().await {
                        match msg_result {
                            Ok(Message::Text(text)) => {
                                debug!("📨 WS message: {}", &text[..text.len().min(200)]);
                                
                                if let Some(event) = parser(&text) {
                                    if tx.send(event).await.is_err() {
                                        info!("📪 Receiver dropped, stopping subscription");
                                        return;
                                    }
                                }
                            }
                            Ok(Message::Ping(data)) => {
                                let _ = write.send(Message::Pong(data)).await;
                            }
                            Ok(Message::Close(_)) => {
                                warn!("🔌 WebSocket closed by server");
                                break;
                            }
                            Err(e) => {
                                error!("❌ WebSocket error: {}", e);
                                break;
                            }
                            _ => {}
                        }
                    }

                    is_connected.store(false, Ordering::SeqCst);
                    let _ = tx.send(WsEvent::Disconnected).await;
                }
                Err(e) => {
                    error!("❌ WebSocket connection failed: {}", e);
                    let _ = tx.send(WsEvent::Error(e.to_string())).await;
                }
            }

            // Reconnection logic with exponential backoff
            reconnect_attempts += 1;
            if reconnect_attempts >= WS_MAX_RECONNECT_ATTEMPTS {
                error!("❌ Max reconnection attempts reached, giving up");
                let _ = tx
                    .send(WsEvent::Error("Max reconnection attempts reached".to_string()))
                    .await;
                return;
            }

            warn!(
                "🔄 Reconnecting in {}ms (attempt {}/{})",
                reconnect_delay, reconnect_attempts, WS_MAX_RECONNECT_ATTEMPTS
            );
            tokio::time::sleep(std::time::Duration::from_millis(reconnect_delay)).await;

            // Exponential backoff with cap
            reconnect_delay = (reconnect_delay * 2).min(WS_RECONNECT_MAX_MS);
        }
    }

    // ============================================
    // PARSERS
    // ============================================

    fn parse_new_head(msg: &str) -> Option<WsEvent> {
        let json: serde_json::Value = serde_json::from_str(msg).ok()?;
        
        // Check if it's a subscription result
        if let Some(result) = json.get("params").and_then(|p| p.get("result")) {
            let header: BlockHeader = serde_json::from_value(result.clone()).ok()?;
            return Some(WsEvent::NewBlock(header));
        }
        
        None
    }

    fn parse_log(msg: &str) -> Option<WsEvent> {
        let json: serde_json::Value = serde_json::from_str(msg).ok()?;
        
        if let Some(result) = json.get("params").and_then(|p| p.get("result")) {
            let log: LogEvent = serde_json::from_value(result.clone()).ok()?;
            return Some(WsEvent::Log(log));
        }
        
        None
    }

    fn parse_pending_tx(msg: &str) -> Option<WsEvent> {
        let json: serde_json::Value = serde_json::from_str(msg).ok()?;
        
        if let Some(result) = json.get("params").and_then(|p| p.get("result")) {
            let tx: PendingTransaction = serde_json::from_value(result.clone()).ok()?;
            return Some(WsEvent::PendingTx(tx));
        }
        
        None
    }

    fn parse_mined_tx(msg: &str) -> Option<WsEvent> {
        let json: serde_json::Value = serde_json::from_str(msg).ok()?;
        
        if let Some(result) = json.get("params").and_then(|p| p.get("result")) {
            let tx: MinedTransaction = serde_json::from_value(result.clone()).ok()?;
            return Some(WsEvent::MinedTx(tx));
        }
        
        None
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.is_connected.load(Ordering::SeqCst)
    }

    /// Get chain ID
    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }
}


// ============================================
// SNIPER BOT HELPER - NEW TOKEN DETECTOR
// ============================================

/// New Token Detector - Optimized for sniper bot
/// 
/// Monitors PairCreated events from DEX factories to detect new tokens
/// INSTANTLY (vs polling which has 3-10 second delay)
pub struct NewTokenDetector {
    ws_client: AlchemyWsClient,
}

impl NewTokenDetector {
    /// Create new token detector for a chain
    pub fn new(chain_id: u64) -> Result<Self> {
        let ws_client = AlchemyWsClient::new(chain_id)?;
        Ok(Self { ws_client })
    }

    /// Start monitoring for new token pairs
    /// 
    /// Returns receiver for PairCreated events
    /// Each event contains: token0, token1, pair address
    pub async fn start(&self) -> Result<mpsc::Receiver<NewPairEvent>> {
        let (tx, rx) = mpsc::channel(100);
        
        // Subscribe to PairCreated logs
        let mut log_rx = self.ws_client.subscribe_logs(LogFilter::pair_created()).await?;

        tokio::spawn(async move {
            while let Some(event) = log_rx.recv().await {
                if let WsEvent::Log(log) = event {
                    if let Some(pair_event) = Self::parse_pair_created(&log) {
                        if tx.send(pair_event).await.is_err() {
                            break;
                        }
                    }
                }
            }
        });

        Ok(rx)
    }

    /// Parse PairCreated event from log
    fn parse_pair_created(log: &LogEvent) -> Option<NewPairEvent> {
        // PairCreated(address indexed token0, address indexed token1, address pair, uint)
        // topics[0] = event signature
        // topics[1] = token0 (indexed)
        // topics[2] = token1 (indexed)
        // data = pair address + uint
        
        if log.topics.len() < 3 {
            return None;
        }

        let token0 = Self::topic_to_address(&log.topics[1])?;
        let token1 = Self::topic_to_address(&log.topics[2])?;
        
        // Pair address is first 32 bytes of data (padded)
        let pair = if log.data.len() >= 66 {
            format!("0x{}", &log.data[26..66])
        } else {
            return None;
        };

        Some(NewPairEvent {
            factory: log.address.clone(),
            token0,
            token1,
            pair,
            block_number: log.block_number.clone(),
            tx_hash: log.transaction_hash.clone(),
        })
    }

    /// Convert topic (32 bytes) to address (20 bytes)
    fn topic_to_address(topic: &str) -> Option<String> {
        // Topic is 0x + 64 hex chars, address is last 40 chars
        if topic.len() >= 42 {
            Some(format!("0x{}", &topic[topic.len() - 40..]))
        } else {
            None
        }
    }
}

/// New pair creation event
#[derive(Debug, Clone)]
pub struct NewPairEvent {
    pub factory: String,
    pub token0: String,
    pub token1: String,
    pub pair: String,
    pub block_number: String,
    pub tx_hash: String,
}

impl NewPairEvent {
    /// Get the "new" token (not WETH/WBNB)
    pub fn get_new_token(&self, weth_address: &str) -> Option<&str> {
        let weth_lower = weth_address.to_lowercase();
        if self.token0.to_lowercase() == weth_lower {
            Some(&self.token1)
        } else if self.token1.to_lowercase() == weth_lower {
            Some(&self.token0)
        } else {
            // Neither is WETH, return token0 as default
            Some(&self.token0)
        }
    }
}

// ============================================
// MULTI-CHAIN WEBSOCKET MANAGER
// ============================================

/// Multi-chain WebSocket manager
/// 
/// Manages WebSocket connections across multiple chains
pub struct WsManager {
    clients: std::collections::HashMap<u64, AlchemyWsClient>,
}

impl WsManager {
    /// Create manager with specified chains
    pub fn new(chain_ids: &[u64]) -> Self {
        let mut clients = std::collections::HashMap::new();

        for &chain_id in chain_ids {
            match AlchemyWsClient::new(chain_id) {
                Ok(client) => {
                    info!("✅ WebSocket client ready for chain {}", chain_id);
                    clients.insert(chain_id, client);
                }
                Err(e) => {
                    warn!("⚠️ Failed to create WebSocket client for chain {}: {}", chain_id, e);
                }
            }
        }

        Self { clients }
    }

    /// Get client for a chain
    pub fn get(&self, chain_id: u64) -> Option<&AlchemyWsClient> {
        self.clients.get(&chain_id)
    }

    /// Get all connected chain IDs
    pub fn connected_chains(&self) -> Vec<u64> {
        self.clients
            .iter()
            .filter(|(_, c)| c.is_connected())
            .map(|(id, _)| *id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_filter_pair_created() {
        let filter = LogFilter::pair_created();
        assert!(filter.topics.is_some());
        let topics = filter.topics.unwrap();
        assert_eq!(topics.len(), 1);
        assert!(topics[0].as_ref().unwrap().starts_with("0x0d3648bd"));
    }

    #[test]
    fn test_log_filter_transfer() {
        let filter = LogFilter::transfer();
        assert!(filter.topics.is_some());
        let topics = filter.topics.unwrap();
        assert!(topics[0].as_ref().unwrap().starts_with("0xddf252ad"));
    }

    #[test]
    fn test_pending_tx_filter() {
        let filter = PendingTxFilter::to_address("0x1234567890123456789012345678901234567890");
        assert!(filter.to_address.is_some());
        assert!(filter.from_address.is_none());
    }

    #[test]
    fn test_topic_to_address() {
        let topic = "0x000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";
        let address = NewTokenDetector::topic_to_address(topic);
        assert_eq!(address, Some("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2".to_string()));
    }

    #[test]
    fn test_new_pair_event_get_new_token() {
        let event = NewPairEvent {
            factory: "0xfactory".to_string(),
            token0: "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2".to_string(), // WETH
            token1: "0xnewtoken".to_string(),
            pair: "0xpair".to_string(),
            block_number: "12345".to_string(),
            tx_hash: "0xhash".to_string(),
        };

        let weth = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";
        let new_token = event.get_new_token(weth);
        assert_eq!(new_token, Some("0xnewtoken"));
    }
}
