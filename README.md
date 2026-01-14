# 🛡️ Ruster Shield

<div align="center">

**High-Performance Multi-Chain Token Risk Analyzer**

*Powered by Rust REVM + Alchemy*

[![Rust CI](https://github.com/nirvagold/Rust-REVM/actions/workflows/rust-ci.yml/badge.svg)](https://github.com/nirvagold/Rust-REVM/actions)
[![Docker](https://img.shields.io/docker/pulls/septianff73/ruster-api)](https://hub.docker.com/r/septianff73/ruster-api)
[![Rust](https://img.shields.io/badge/rust-nightly-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

[**🌐 Live Demo**](https://nirvagold.github.io/Rust-REVM/) • [**📖 API Docs**](#-api-endpoints) • [**🐳 Docker**](https://hub.docker.com/r/septianff73/ruster-api)

</div>

---

## ⚡ What is Ruster Shield?

Ruster Shield is a **real-time honeypot detection API** that protects traders from scam tokens. It uses the REVM (Rust EVM) to simulate buy/sell transactions before you trade, detecting:

- 🍯 **Honeypot tokens** (can buy, can't sell)
- 💸 **High tax tokens** (excessive buy/sell fees)
- � **Blacklist functions** (owner ca n block your wallet)
- 🔒 **Trading restrictions** (max tx, cooldowns, etc.)

---

## 🌐 Supported Chains (8 Total)

| Chain | ID | Native | Status |
|-------|-----|--------|--------|
| 🔷 Ethereum | 1 | ETH | ✅ Full Support |
| 🟡 BSC | 56 | BNB | ✅ Full Support |
| 🟣 Polygon | 137 | MATIC | ✅ Full Support |
| 🔵 Arbitrum | 42161 | ETH | ✅ Full Support |
| 🔴 Optimism | 10 | ETH | ✅ Full Support |
| 🔵 Base | 8453 | ETH | ✅ Full Support |
| 🔺 Avalanche | 43114 | AVAX | ✅ Full Support |
| 🟢 **Solana** | 900 | SOL | ✅ DexScreener + DAS API |

---

## 🚀 Quick Start

### Option 1: Use Public API (Recommended)

```bash
curl -X POST https://yelling-patience-nirvagold-0a943e82.koyeb.app/v1/honeypot/check \
  -H "Content-Type: application/json" \
  -d '{"token_address": "0xdAC17F958D2ee523a2206206994597C13D831ec7", "chain_id": 1}'
```

### Option 2: Docker

```bash
docker pull septianff73/ruster-api:latest

docker run -p 8080:8080 \
  -e ALCHEMY_API_KEY="your_key" \
  septianff73/ruster-api:latest
```

### Option 3: Build from Source

```bash
git clone https://github.com/nirvagold/Rust-REVM.git
cd Rust-REVM

# Setup environment
cp .env.example .env
# Edit .env with your ALCHEMY_API_KEY

# Build & Run
cargo build --release
cargo run --release --bin ruster_api
```

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

### Honeypot Check

```bash
# EVM Token (auto-detect chain)
curl -X POST https://yelling-patience-nirvagold-0a943e82.koyeb.app/v1/honeypot/check \
  -H "Content-Type: application/json" \
  -d '{"token_address": "0x...", "chain_id": 56}'

# Solana Token
curl -X POST https://yelling-patience-nirvagold-0a943e82.koyeb.app/v1/honeypot/check \
  -H "Content-Type: application/json" \
  -d '{"token_address": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"}'
```

### Response Example

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
    "price_usd": "0.001234",
    "dex_name": "PancakeSwap V2",
    "reason": "Token passed buy/sell simulation"
  },
  "latency_ms": 245.5
}
```

---

## 📊 Risk Score Levels

| Score | Level | Recommendation |
|-------|-------|----------------|
| 0-20 | ✅ **SAFE** | Low risk, proceed with caution |
| 21-40 | 🟡 **LOW** | Some concerns, DYOR |
| 41-60 | 🟠 **MEDIUM** | Elevated risk, review carefully |
| 61-80 | 🔴 **HIGH** | Likely to lose funds |
| 81-100 | 💀 **CRITICAL** | Confirmed honeypot/scam |

---

## 🐍 SDK Examples

### Python

```python
import requests

API_URL = "https://yelling-patience-nirvagold-0a943e82.koyeb.app"

def check_token(address: str, chain_id: int = 0) -> dict:
    """Check if token is honeypot. chain_id=0 for auto-detect."""
    response = requests.post(
        f"{API_URL}/v1/honeypot/check",
        json={"token_address": address, "chain_id": chain_id}
    )
    return response.json()

# Example: Check BSC token
result = check_token("0x...", chain_id=56)
if result["success"]:
    data = result["data"]
    print(f"Token: {data['token_symbol']}")
    print(f"Honeypot: {data['is_honeypot']}")
    print(f"Risk: {data['risk_score']}/100")
    print(f"Tax: {data['total_loss_percent']:.2f}%")
```

### JavaScript/TypeScript

```typescript
const API_URL = "https://yelling-patience-nirvagold-0a943e82.koyeb.app";

async function checkToken(address: string, chainId = 0) {
  const response = await fetch(`${API_URL}/v1/honeypot/check`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ token_address: address, chain_id: chainId })
  });
  return response.json();
}

// Example
const result = await checkToken("0x...", 56);
console.log(`Risk Score: ${result.data.risk_score}`);
```

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
│                     │    REVM     │                            │
│                     │  Simulator  │                            │
│                     └─────────────┘                            │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## 📁 Project Structure

```
ruster-shield/
├── src/
│   ├── api/              # REST API (Axum)
│   │   ├── handlers.rs   # Request handlers
│   │   ├── routes.rs     # Route definitions
│   │   └── types.rs      # Request/Response types
│   ├── core/             # Business Logic
│   │   ├── honeypot.rs   # PERS Algorithm
│   │   ├── ml_risk.rs    # ML-based scoring
│   │   └── risk_score.rs # Risk calculation
│   ├── providers/        # Data Sources
│   │   ├── rpc.rs        # Multi-chain RPC
│   │   ├── alchemy.rs    # Alchemy APIs
│   │   ├── solana.rs     # Solana RPC + DAS
│   │   └── dexscreener.rs
│   ├── models/           # Data Structures
│   └── utils/            # Utilities
├── docs/                 # GitHub Pages
├── examples/             # Demo scripts
└── tests/                # Test files
```

---

## 🔧 Key Features

### PERS Algorithm (Pre-Execution Risk Scoring)
1. Fetch token bytecode from RPC
2. Generate random caller address (bypass whitelist)
3. Simulate BUY transaction (ETH → Token)
4. Simulate APPROVE (Token → Router)
5. Simulate SELL transaction (Token → ETH)
6. If SELL reverts → **HONEYPOT DETECTED**
7. Scan bytecode for blacklist/setBots functions

### ML-Based Risk Scoring
- Liquidity analysis (locked LP, pool count)
- Trading patterns (volume, holder distribution)
- Social signals (age, website, socials)
- Historical scam detection

### Alchemy Best Practices
- Gzip compression (75% faster for large responses)
- Exponential backoff with jitter
- Batch requests (max 50 per batch)
- Concurrent request handling

---

## 📈 Tech Stack

| Component | Technology |
|-----------|------------|
| Language | Rust (nightly) |
| EVM Simulator | REVM 18 |
| Web Framework | Axum 0.7 |
| Async Runtime | Tokio |
| RPC Client | Alloy 0.8 |
| Cache | DashMap (5min TTL) |
| Deployment | Docker + Koyeb |
| CI/CD | GitHub Actions |

---

## ⚠️ Disclaimer

**IMPORTANT: Please read our [Terms of Service](TERMS_OF_SERVICE.md)**

- This API is for **informational purposes only**
- Risk scores are **estimates** and may not be accurate
- Tokens can change behavior after analysis
- **Always DYOR** (Do Your Own Research)
- We are **NOT liable** for any trading losses
- **Never invest more than you can afford to lose**

---

## 🤝 Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

```bash
# Run tests
cargo test --lib

# Run clippy
cargo clippy --all-targets -- -D warnings

# Format code
cargo fmt
```

---

## 📄 License

MIT © 2026 [nirvagold](https://github.com/nirvagold)

---

<div align="center">

**Built with 🦀 Rust + ⚡ REVM + 🔮 Alchemy**

[Live Demo](https://nirvagold.github.io/Rust-REVM/) • [Docker Hub](https://hub.docker.com/r/septianff73/ruster-api) • [GitHub](https://github.com/nirvagold/Rust-REVM)

⭐ Star this repo if you find it useful!

</div>
