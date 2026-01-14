# 🛡️ Ruster Shield - Project Summary

## Overview

**Ruster Shield** adalah high-performance token risk analyzer yang menggunakan Rust REVM untuk mendeteksi honeypot tokens secara real-time. Proyek ini terdiri dari 2 komponen utama:

1. **Rust API** (Public) - REST API untuk honeypot detection
2. **Python Sniper Bot** (Private) - Real-time token scanner dengan Telegram alerts

---

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        RUSTER SHIELD                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────────────┐ │
│  │   Clients   │───▶│  Rust API   │───▶│   Blockchain RPC    │ │
│  │  Web/Bot    │    │   (Axum)    │    │  (Alchemy/Public)   │ │
│  └─────────────┘    └─────────────┘    └─────────────────────┘ │
│                            │                                    │
│                     ┌──────┴──────┐                            │
│                     │   REVM      │                            │
│                     │  Simulator  │                            │
│                     └─────────────┘                            │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              PRIVATE: Python Sniper Bot                  │   │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐    │   │
│  │  │WebSocket│─▶│ Detect  │─▶│ Analyze │─▶│Telegram │    │   │
│  │  │Listener │  │New Pair │  │via API  │  │ Alert   │    │   │
│  │  └─────────┘  └─────────┘  └─────────┘  └─────────┘    │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## 📁 Project Structure

```
ruster-shield/
├── src/                          # Rust Source Code
│   ├── api/                      # REST API Layer
│   │   ├── handlers.rs           # Request handlers
│   │   ├── middleware.rs         # Logging, CORS
│   │   ├── routes.rs             # Route definitions
│   │   └── types.rs              # API request/response types
│   │
│   ├── core/                     # Business Logic
│   │   ├── honeypot.rs           # PERS Algorithm - Buy/Sell simulation
│   │   ├── ml_risk.rs            # ML-based risk scoring
│   │   ├── risk_score.rs         # Risk score calculation
│   │   ├── analyzer.rs           # Mempool analyzer
│   │   └── simulator.rs          # REVM simulator
│   │
│   ├── providers/                # Data Sources
│   │   ├── rpc.rs                # Multi-chain RPC provider
│   │   ├── alchemy.rs            # Alchemy-specific APIs
│   │   ├── solana.rs             # Solana RPC + DAS API
│   │   ├── dexscreener.rs        # DexScreener integration
│   │   ├── websocket.rs          # WebSocket subscriptions
│   │   └── trace.rs              # Debug trace analysis
│   │
│   ├── models/                   # Data Structures
│   │   ├── config.rs             # Chain configurations
│   │   ├── errors.rs             # Error types & codes
│   │   └── types.rs              # Common types
│   │
│   ├── utils/                    # Utilities
│   │   ├── cache.rs              # In-memory cache (5min TTL)
│   │   ├── constants.rs          # Chain IDs, addresses
│   │   ├── decoder.rs            # Swap decoder
│   │   └── telemetry.rs          # Stats collection
│   │
│   ├── lib.rs                    # Library exports
│   └── main.rs                   # CLI entry point
│
├── bot/                          # 🔒 PRIVATE - Python Sniper Bot
│   ├── multichain_sniper.py      # Multi-chain real-time scanner
│   ├── realtime_sniper.py        # WebSocket-based detection
│   ├── trading_bot.py            # Auto-trading logic
│   └── requirements.txt          # Python dependencies
│
├── docs/                         # GitHub Pages
│   └── index.html                # Interactive web UI
│
├── tests/                        # Test files
├── examples/                     # Demo scripts
└── .github/workflows/            # CI/CD
```

---

## 🌐 Supported Chains (8 Total)

| Chain | ID | Native | DEXes |
|-------|-----|--------|-------|
| Ethereum | 1 | ETH | Uniswap V2, SushiSwap |
| BSC | 56 | BNB | PancakeSwap V2, BiSwap |
| Polygon | 137 | MATIC | QuickSwap, SushiSwap |
| Arbitrum | 42161 | ETH | Camelot, SushiSwap |
| Optimism | 10 | ETH | Velodrome |
| Base | 8453 | ETH | BaseSwap, Aerodrome |
| Avalanche | 43114 | AVAX | TraderJoe, Pangolin |
| **Solana** | 900 | SOL | Raydium, Orca, pump.fun |

---

## 🔧 Key Features

### 1. PERS Algorithm (Pre-Execution Risk Scoring)

```
1. Fetch bytecode from RPC
2. Generate random caller (prevent whitelist bypass)
3. Simulate BUY (ETH → Token)
4. Simulate APPROVE (Token → Router)
5. Simulate SELL (Token → ETH)
6. If SELL reverts → HONEYPOT (risk_score = 100)
7. Scan bytecode for blacklist/setBots functions
```

### 2. ML-Based Risk Scoring
- Liquidity features (locked, pool count, LP holders)
- Trading features (volume, holder count, price change)
- Social features (age, website, twitter, telegram)
- Historical patterns (scam detection)

### 3. Multi-Chain Support
- Single `ALCHEMY_API_KEY` for all chains
- Auto-detect chain from token address
- Fallback to public RPCs

### 4. Real-Time Detection (Private Bot)
- WebSocket subscription to PairCreated events
- Instant honeypot analysis
- Telegram alerts with buy buttons
- Filter: max risk 50, min liquidity $1000

---

## 📡 API Endpoints

**Base URL:** `https://yelling-patience-nirvagold-0a943e82.koyeb.app`

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/health` | GET | Health check |
| `/v1/stats` | GET | API statistics |
| `/v1/honeypot/check` | POST | Honeypot detection |
| `/v1/analyze/token` | POST | Full risk analysis |
| `/v1/analyze/batch` | POST | Batch (max 100 tokens) |

### Example Request
```bash
curl -X POST https://yelling-patience-nirvagold-0a943e82.koyeb.app/v1/honeypot/check \
  -H "Content-Type: application/json" \
  -d '{"token_address": "0x...", "chain_id": 56}'
```

### Example Response
```json
{
  "success": true,
  "data": {
    "token_address": "0x...",
    "token_name": "Example Token",
    "token_symbol": "EXT",
    "chain_id": 56,
    "chain_name": "BNB Smart Chain",
    "is_honeypot": false,
    "risk_score": 15,
    "buy_tax_percent": 0.5,
    "sell_tax_percent": 0.5,
    "total_loss_percent": 1.0,
    "liquidity_usd": 50000,
    "price_usd": "0.001234"
  }
}
```

---

## 🚀 Deployment

### Koyeb (Production)
- Auto-deploy from GitHub master branch
- Docker image built from `Dockerfile`
- Environment: `ALCHEMY_API_KEY`

### Local Development
```bash
# Start API
export ALCHEMY_API_KEY=your_key
cargo run --release --bin ruster_api

# Start Sniper Bot (separate terminal)
cd bot
python multichain_sniper.py
```

---

## 📊 Risk Score Levels

| Score | Level | Action |
|-------|-------|--------|
| 0-20 | ✅ SAFE | Trade freely |
| 21-40 | 🟡 LOW | Proceed with caution |
| 41-60 | 🟠 MEDIUM | Review before trading |
| 61-80 | 🔴 HIGH | Likely to lose funds |
| 81-100 | 💀 CRITICAL | Do not trade |

---

## 🔒 Private Components (Not in GitHub)

| Component | Description |
|-----------|-------------|
| `/bot/` | Python sniper bots with trading logic |
| `.env` | API keys (ALCHEMY, TELEGRAM) |
| `tests/*.py` | Python integration tests |
| `/telemetry/` | Stats and logs |

---

## 📈 Tech Stack

| Layer | Technology |
|-------|------------|
| Language | Rust (nightly) |
| EVM Simulator | REVM 18 |
| Web Framework | Axum 0.7 |
| Async Runtime | Tokio |
| RPC Client | Alloy 0.8 |
| Cache | DashMap |
| Bot | Python 3.11 + aiohttp |
| Deployment | Docker + Koyeb |
| CI/CD | GitHub Actions |

---

## 📝 Recent Updates

1. **Solana Support** - Full integration with DAS API
2. **ML Risk Scoring** - Weighted feature analysis
3. **RPC-First Metadata** - Token name/symbol from blockchain (no DexScreener delay)
4. **Multi-Chain Sniper** - 8 chains + Solana real-time detection
5. **Alchemy Best Practices** - Gzip, batch, exponential backoff

---

## 👤 Author

**nirvagold** - [GitHub](https://github.com/nirvagold)

---

*Last updated: January 2026*
