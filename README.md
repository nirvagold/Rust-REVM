# Ruster Shield 🛡️

<div align="center">

**High-Performance Token Risk Analyzer powered by Rust REVM**

[![Rust CI](https://github.com/nirvagold/Rust-REVM/actions/workflows/rust-ci.yml/badge.svg)](https://github.com/nirvagold/Rust-REVM/actions)
[![Docker](https://img.shields.io/docker/pulls/septianff73/ruster-api)](https://hub.docker.com/r/septianff73/ruster-api)
[![Rust](https://img.shields.io/badge/rust-nightly-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

[**🌐 Live Demo**](https://nirvagold.github.io/Rust-REVM/) • [**📖 API Docs**](https://nirvagold.github.io/Rust-REVM/) • [**🐳 Docker Hub**](https://hub.docker.com/r/septianff73/ruster-api)

</div>

---

## ⚡ Features

| Feature | Description |
|---------|-------------|
| 🍯 **Honeypot Detection** | Simulates buy/sell via `eth_call` on real blockchain state |
| 💰 **Tax Analysis** | Calculates exact buy/sell tax from price quotes |
| 🔍 **Access Control Scan** | Detects blacklist/setBots functions in bytecode |
| 💾 **In-Memory Cache** | 5-min TTL cache to reduce RPC costs |
| 📦 **Batch Processing** | Analyze up to 100 tokens in parallel |
| 🌐 **REST API** | Production-ready with CORS support |

---

## 🚀 Quick Start

### Option 1: Docker (Recommended)

```bash
# Pull from Docker Hub
docker pull septianff73/ruster-api:latest

# Run
docker run -p 3000:3000 \
  -e ETH_HTTP_URL="https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY" \
  septianff73/ruster-api:latest
```

### Option 2: Docker Compose

```yaml
services:
  ruster-api:
    image: septianff73/ruster-api:latest
    ports:
      - "3000:3000"
    environment:
      - ETH_HTTP_URL=https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY
      - RUST_LOG=info
```

```bash
docker-compose up -d
```

### Option 3: Build from Source

```bash
git clone https://github.com/nirvagold/Rust-REVM.git
cd Rust-REVM
cp .env.example .env  # Edit with your RPC URL
cargo build --release
cargo run --release --bin ruster_api
```

---

## 🎯 Try the Live Demo

👉 **[nirvagold.github.io/Rust-REVM](https://nirvagold.github.io/Rust-REVM/)**

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

## 🌐 API Endpoints

Base URL: `https://yelling-patience-nirvagold-0a943e82.koyeb.app`

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/health` | GET | Health check |
| `/v1/stats` | GET | API statistics |
| `/v1/honeypot/check` | POST | Honeypot detection |
| `/v1/analyze/token` | POST | Full risk analysis |
| `/v1/analyze/batch` | POST | Batch analysis (max 100) |

### Example: Honeypot Check

```bash
curl -X POST https://yelling-patience-nirvagold-0a943e82.koyeb.app/v1/honeypot/check \
  -H "Content-Type: application/json" \
  -d '{"token_address": "0xdAC17F958D2ee523a2206206994597C13D831ec7"}'
```

**Response:**
```json
{
  "success": true,
  "data": {
    "is_honeypot": false,
    "risk_score": 10,
    "buy_tax_percent": 0.30,
    "sell_tax_percent": 0.30,
    "total_loss_percent": 0.60,
    "reason": "Token passed buy/sell simulation"
  }
}
```

---

## 🐍 Python Example

```python
import requests

def check_honeypot(token: str) -> dict:
    r = requests.post(
        "https://yelling-patience-nirvagold-0a943e82.koyeb.app/v1/honeypot/check",
        json={"token_address": token}
    )
    return r.json()

result = check_honeypot("0xdAC17F958D2ee523a2206206994597C13D831ec7")
if result["data"]["is_honeypot"]:
    print("🚨 HONEYPOT!")
else:
    print(f"✅ Safe - Tax: {result['data']['total_loss_percent']:.2f}%")
```

---

## 🏗️ Architecture

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Client    │────▶│  Axum API   │────▶│  Ethereum   │
│  (Web/CLI)  │     │   Server    │     │    RPC      │
└─────────────┘     └─────────────┘     └─────────────┘
                           │
                    ┌──────┴──────┐
                    │   Cache     │
                    │  (DashMap)  │
                    └─────────────┘
```

---

## 📁 Project Structure

```
├── src/
│   ├── api/           # REST API (Axum)
│   ├── cache.rs       # In-memory caching
│   ├── honeypot.rs    # Detection logic
│   └── risk_score.rs  # PERS algorithm
├── docs/              # GitHub Pages
└── tools/             # CLI tools
```

---

## 🤝 Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

---

## 📄 License

MIT © 2026 [nirvagold](https://github.com/nirvagold)

---

<div align="center">

**Built with 🦀 Rust + ⚡ REVM**

[Live Demo](https://nirvagold.github.io/Rust-REVM/) • [Docker Hub](https://hub.docker.com/r/septianff73/ruster-api) • [GitHub](https://github.com/nirvagold/Rust-REVM)

</div>
