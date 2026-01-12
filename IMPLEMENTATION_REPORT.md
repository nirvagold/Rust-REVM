# Ruster Shield - Implementation Report
## Token Risk Analyzer with REVM Simulation

**Project:** Ruster Shield (Rust-REVM)  
**Date:** January 2026  
**Branch:** `feature/token-info`  
**Live API:** https://yelling-patience-nirvagold-0a943e82.koyeb.app  
**Website:** https://nirvagold.github.io/Rust-REVM/

---

## Executive Summary

Ruster Shield adalah API untuk mendeteksi honeypot token dan menganalisis risiko trading di blockchain EVM. Sistem menggunakan simulasi `eth_call` untuk mensimulasikan buy/sell secara real-time pada state blockchain aktual.

### Key Achievements
- ✅ Multi-chain support (7 chains)
- ✅ Multi-DEX support per chain
- ✅ Auto chain detection via DexScreener
- ✅ In-memory caching dengan TTL 5 menit
- ✅ Token metadata (name, symbol, decimals)
- ✅ V2/V3 DEX compatibility detection

---

## 1. Core Architecture

### 1.1 Technology Stack
```
Backend:    Rust + Axum (async web framework)
Simulation: eth_call RPC (NOT local REVM with mock bytecode)
Caching:    DashMap (concurrent HashMap)
API:        DexScreener (route discovery)
Hosting:    Koyeb (Docker container)
```

### 1.2 Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| `eth_call` over local REVM | Accurate simulation on real blockchain state |
| DexScreener for discovery | Auto-detect chain & find DEX with liquidity |
| REVM for security analysis | Real-time, no delay (unlike DexScreener) |
| V2-only router support | V3/Velodrome have incompatible interfaces |

---

## 2. Features Implemented

### 2.1 Honeypot Detection (`src/honeypot.rs`)

**Algorithm: PERS (Pre-Execution Risk Scoring)**

```
1. Fetch token bytecode from RPC
2. Scan for access control functions (blacklist, setBots)
3. Simulate BUY: WETH → Token via getAmountsOut
4. Simulate SELL: Token → WETH via getAmountsOut
5. Calculate tax: (input - output) / input * 100
6. If loss > 90% → HONEYPOT
```

**Key Functions:**
- `detect_async()` - Main detection with multi-DEX support
- `get_amounts_out_with_router()` - Simulate swap quote
- `fetch_token_info()` - Get ERC20 name/symbol/decimals
- `scan_access_control_functions()` - Detect blacklist patterns

**Access Control Detection:**
```rust
// Function selectors scanned:
"setBots", "setBot", "blacklistAddress", "addToBlacklist",
"isBot", "setBlacklist", "addBot", "delBot",
"setTradingEnabled", "setMaxTxAmount", "setMaxWalletSize"
```

### 2.2 Multi-Chain Support (`src/config.rs`)

**Supported Chains:**
| Chain | ID | Native | WETH Address |
|-------|-----|--------|--------------|
| Ethereum | 1 | ETH | 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2 |
| BNB Smart Chain | 56 | BNB | 0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c |
| Polygon | 137 | MATIC | 0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270 |
| Arbitrum | 42161 | ETH | 0x82aF49447D8a07e3bd95BD0d56f35241523fBab1 |
| Optimism | 10 | ETH | 0x4200000000000000000000000000000000000006 |
| Avalanche | 43114 | AVAX | 0xB31f66AA3C1e785363F0875A1B74E27b85FD66c7 |
| Base | 8453 | ETH | 0x4200000000000000000000000000000000000006 |

**RPC Configuration:**
- Single `ALCHEMY_API_KEY` env var generates URLs for all chains
- Fallback to public RPCs if Alchemy not configured
- Per-chain env vars supported: `ETH_HTTP_URL`, `BSC_HTTP_URL`, etc.

### 2.3 Multi-DEX Support

**V2-Compatible Routers Only:**
```
Ethereum:  Uniswap V2, SushiSwap
BSC:       PancakeSwap V2, BiSwap
Polygon:   QuickSwap, SushiSwap
Arbitrum:  Camelot, SushiSwap
Optimism:  SushiSwap (Velodrome removed - not V2 compatible)
Avalanche: TraderJoe, Pangolin
Base:      PancakeSwap V2, BaseSwap, SushiSwap (Aerodrome removed)
```

**Why V2 Only:**
- V3 uses Quoter contract with different interface
- Velodrome/Aerodrome use `route[]` struct parameter
- Our `getAmountsOut(uint256, address[])` only works with V2

### 2.4 DexScreener Integration (`src/dexscreener.rs`)

**Purpose:** Route Discovery (NOT security analysis)

```
✅ USED FOR:
- Auto-detect which chain token is on
- Find DEX with highest liquidity
- Token metadata (name, symbol) as fallback
- Detect V3-only tokens

❌ NOT USED FOR:
- Honeypot detection (has 5-30s delay)
- Tax calculation
- Real-time price for trading
```

**API Endpoint:**
```
GET https://api.dexscreener.com/latest/dex/tokens/{address}
```

**V2 Compatibility Check:**
```rust
pub fn is_v2_compatible(&self) -> bool {
    let is_v3 = self.labels.iter().any(|l| l.contains("v3") || l.contains("v4"));
    let is_velodrome_style = matches!(
        self.dex_id.to_lowercase().as_str(),
        "velodrome" | "aerodrome" | "ramses" | "thena" | "equalizer"
    );
    !is_v3 && !is_velodrome_style
}
```

### 2.5 In-Memory Caching (`src/cache.rs`)

**Implementation:**
```rust
pub struct HoneypotCache {
    cache: DashMap<String, CachedResult>,
    ttl: Duration,  // 5 minutes
    hits: AtomicU64,
    misses: AtomicU64,
}
```

**Features:**
- TTL: 5 minutes
- Cache key: `{chain_id}:{token_address_lowercase}`
- Background cleanup every 60 seconds
- Failed results NOT cached (per CEO directive)
- Hit/miss statistics for monitoring

### 2.6 API Endpoints (`src/api/`)

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/v1/health` | Health check |
| POST | `/v1/honeypot/check` | Honeypot detection |
| POST | `/v1/analyze/token` | Full risk analysis |
| POST | `/v1/analyze/batch` | Batch analysis (max 100) |
| GET | `/v1/stats` | API statistics |

**Request Example:**
```json
POST /v1/honeypot/check
{
  "token_address": "0x...",
  "chain_id": 0  // 0 = auto-detect
}
```

**Response Example:**
```json
{
  "success": true,
  "data": {
    "token_address": "0x...",
    "token_name": "Token Name",
    "token_symbol": "TKN",
    "token_decimals": 18,
    "chain_id": 1,
    "chain_name": "Ethereum",
    "native_symbol": "ETH",
    "is_honeypot": false,
    "risk_score": 15,
    "buy_tax_percent": 0.5,
    "sell_tax_percent": 0.5,
    "total_loss_percent": 1.0,
    "reason": "Token passed buy/sell simulation"
  }
}
```

---

## 3. Risk Score Algorithm

### 3.1 Score Calculation
```rust
fn calculate_risk_score(result: &HoneypotResult) -> u8 {
    // No liquidity = unknown (30)
    if !result.buy_success && !result.sell_success && !result.is_honeypot {
        return 30;
    }

    let base_score = if result.sell_reverted { 100 }
        else if result.is_honeypot { 95 }
        else if result.total_loss_percent > 50.0 { 80 }
        else if result.total_loss_percent > 30.0 { 60 }
        else if result.total_loss_percent > 10.0 { 40 }
        else if result.total_loss_percent > 5.0 { 20 }
        else { 10 };

    // Access control penalty only if loss > 5%
    let penalty = if result.total_loss_percent > 5.0 {
        result.access_control_penalty as u32
    } else { 0 };

    (base_score + penalty).min(100) as u8
}
```

### 3.2 Risk Levels
| Score | Level | Action |
|-------|-------|--------|
| 0-20 | ✅ SAFE | Trade freely |
| 21-40 | 🟡 LOW | Proceed with caution |
| 41-60 | 🟠 MEDIUM | Review before trading |
| 61-80 | 🔴 HIGH | Likely to lose funds |
| 81-100 | 💀 CRITICAL | Do not trade |

---

## 4. Website (`docs/index.html`)

### 4.1 Features
- Single input field (no chain selector needed)
- Auto chain detection via DexScreener
- Supported chains shown as badges
- Real-time status display with animations
- Search history (last 5 searches)
- API documentation tab

### 4.2 Status Display Logic
```javascript
// Priority: Honeypot > V3-Only > No Liquidity > Risk Score
if (isHoneypot) → "🚨 HONEYPOT DETECTED" (red)
else if (isV3Only) → "⚠️ V3 ONLY (Not Supported)" (blue)
else if (noLiquidity) → "❓ NO LIQUIDITY" (gray)
else if (riskScore >= 80) → "🚨 CRITICAL RISK" (red)
else if (riskScore >= 50) → "⚠️ HIGH RISK" (yellow)
else if (riskScore >= 30) → "⚠️ MEDIUM RISK" (yellow)
else → "✅ SAFE TO TRADE" (green)
```

---

## 5. Known Limitations

### 5.1 DEX Compatibility
| DEX Type | Supported | Reason |
|----------|-----------|--------|
| Uniswap V2 | ✅ Yes | Standard `getAmountsOut` |
| SushiSwap | ✅ Yes | V2 fork |
| PancakeSwap V2 | ✅ Yes | V2 fork |
| Uniswap V3 | ❌ No | Uses Quoter contract |
| Aerodrome | ❌ No | Velodrome fork, different interface |
| Velodrome | ❌ No | Uses `route[]` struct |
| Balancer | ❌ No | Different AMM model |

### 5.2 Token Limitations
- Tokens only on V3 DEXes cannot be analyzed
- Rebasing tokens may show incorrect tax
- Fee-on-transfer detection is approximate

### 5.3 DexScreener Limitations
- 5-30 second data delay
- Not all tokens indexed
- Rate limits on free tier

---

## 6. File Structure

```
src/
├── api/
│   ├── handlers.rs    # Request handlers
│   ├── routes.rs      # Route definitions
│   └── types.rs       # Request/Response types
├── bin/
│   └── ruster_api.rs  # API binary entry
├── cache.rs           # In-memory caching
├── config.rs          # Chain & DEX configuration
├── dexscreener.rs     # DexScreener API client
├── honeypot.rs        # Core detection logic
├── lib.rs             # Library exports
├── main.rs            # CLI entry
├── risk_score.rs      # Risk scoring algorithm
└── telemetry.rs       # Statistics collection

docs/
└── index.html         # Website

.env.example           # Environment template
Dockerfile             # Container build
docker-compose.yml     # Local development
```

---

## 7. Environment Variables

```bash
# Required for multi-chain
ALCHEMY_API_KEY=your_key_here

# Or individual chain URLs
ETH_HTTP_URL=https://eth-mainnet.g.alchemy.com/v2/KEY
BSC_HTTP_URL=https://bnb-mainnet.g.alchemy.com/v2/KEY
# ... etc

# Server config
PORT=8080
RUST_LOG=info
```

---

## 8. Deployment

### 8.1 Docker
```bash
docker build -t ruster-api .
docker run -p 8080:8080 -e ALCHEMY_API_KEY=xxx ruster-api
```

### 8.2 Koyeb
- Image: `docker.io/septianff73/ruster-api`
- Environment: `ETH_HTTP_URL` with Alchemy RPC
- Port: 8080

---

## 9. Future Improvements

### 9.1 High Priority
- [ ] Uniswap V3 Quoter support
- [ ] Velodrome/Aerodrome router support
- [ ] WebSocket for real-time updates

### 9.2 Medium Priority
- [ ] Redis caching for horizontal scaling
- [ ] Rate limiting per IP
- [ ] API key authentication

### 9.3 Low Priority
- [ ] Solana support
- [ ] Historical analysis
- [ ] Telegram bot integration

---

## 10. API Usage Examples

### Python
```python
import requests

def check_token(address: str) -> dict:
    response = requests.post(
        "https://yelling-patience-nirvagold-0a943e82.koyeb.app/v1/honeypot/check",
        json={"token_address": address}
    )
    return response.json()

result = check_token("0x...")
print(f"Honeypot: {result['data']['is_honeypot']}")
print(f"Risk: {result['data']['risk_score']}/100")
```

### cURL
```bash
curl -X POST https://yelling-patience-nirvagold-0a943e82.koyeb.app/v1/honeypot/check \
  -H "Content-Type: application/json" \
  -d '{"token_address": "0x..."}'
```

---

## 11. Commits Summary

| Commit | Description |
|--------|-------------|
| `feat: token-info` | Add token name/symbol/decimals to response |
| `feat: multi-chain` | Support 7 EVM chains with Alchemy |
| `feat: multi-dex` | Multiple DEX routers per chain |
| `feat: dexscreener` | Auto chain detection |
| `fix: v2-only` | Remove incompatible V3/Velodrome routers |
| `feat: v3-detect` | Show clear message for V3-only tokens |
| `fix: ui-status` | Correct status display logic |

---

## 12. Conclusion

Ruster Shield provides accurate honeypot detection for tokens on V2-compatible DEXes across 7 EVM chains. The hybrid approach using DexScreener for discovery and REVM for security analysis ensures both convenience (auto chain detection) and accuracy (real-time simulation).

**Key Strengths:**
- Real-time simulation on actual blockchain state
- Multi-chain, multi-DEX support
- Clear messaging for unsupported scenarios
- Fast response with caching

**Main Limitation:**
- V3/Velodrome DEXes not supported (different interface)

For tokens only available on V3, users are advised to trade directly on the DEX with appropriate caution.

---

*Report generated: January 2026*  
*Total implementation: ~3000 lines of Rust code*
