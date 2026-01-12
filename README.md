# Ruster Shield 🛡️

<div align="center">

**High-Performance Token Risk Analyzer powered by Rust REVM**

[![Rust CI](https://github.com/nirvagold/Rust-REVM/actions/workflows/rust-ci.yml/badge.svg)](https://github.com/nirvagold/Rust-REVM/actions)
[![Rust](https://img.shields.io/badge/rust-nightly-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![API](https://img.shields.io/badge/API-Live-brightgreen.svg)](https://yelling-patience-nirvagold-0a943e82.koyeb.app/v1/health)

[**🌐 Live Demo**](https://nirvagold.github.io/Rust-REVM/) • [**📖 API Docs**](#-api-endpoints) • [**🐍 Python SDK**](#-python-sdk)

</div>

---

## ⚡ Features

| Feature | Description |
|---------|-------------|
| 🍯 **Honeypot Detection** | Simulates buy/sell via `eth_call` on real blockchain state |
| 💰 **Tax Analysis** | Calculates exact buy/sell tax from price quotes |
| 🔍 **Access Control Scan** | Detects blacklist/setBots functions in bytecode |
| 🚀 **Sub-second Latency** | ~1-2s per check using RPC simulation |
| 📦 **Batch Processing** | Analyze up to 100 tokens in parallel |
| 🌐 **REST API** | Production-ready with CORS support |

---

## 🎯 Quick Start

### Try the Live Demo
👉 **[nirvagold.github.io/Rust-REVM](https://nirvagold.github.io/Rust-REVM/)**

### API Example

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

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/health` | GET | Health check |
| `/v1/honeypot/check` | POST | Honeypot detection |
| `/v1/analyze/token` | POST | Full risk analysis |
| `/v1/analyze/batch` | POST | Batch analysis (max 100) |
| `/v1/stats` | GET | Protection statistics |


---

## 🛠️ Self-Hosting

### Prerequisites
- Rust nightly
- Ethereum RPC (Alchemy/Infura)

### Build & Run

```bash
git clone https://github.com/nirvagold/Rust-REVM.git
cd Rust-REVM

# Configure
cp .env.example .env
# Edit .env with your RPC URL

# Build
cargo build --release

# Run API server
cargo run --release --bin ruster_api
```

### Docker

```bash
docker build -t ruster-shield .
docker run -p 3000:3000 -e ETH_HTTP_URL="https://..." ruster-shield
```

---

## 🐍 Python SDK

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
                    │             │
              ┌─────▼─────┐ ┌─────▼─────┐
              │ Honeypot  │ │   Risk    │
              │ Detector  │ │  Scorer   │
              └───────────┘ └───────────┘
```

---

## 📁 Project Structure

```
├── src/
│   ├── api/           # REST API (Axum)
│   ├── honeypot.rs    # Detection logic
│   ├── risk_score.rs  # PERS algorithm
│   └── telemetry.rs   # Analytics
├── docs/              # GitHub Pages
├── tools/             # CLI tools
└── sdk/python/        # Python SDK
```

---

## 🤝 Contributing

PRs welcome! Please ensure `cargo clippy` passes before submitting.

---

## 📄 License

MIT © 2026 [nirvagold](https://github.com/nirvagold)

---

<div align="center">

**Built with 🦀 Rust + ⚡ REVM**

[Live Demo](https://nirvagold.github.io/Rust-REVM/) • [API](https://yelling-patience-nirvagold-0a943e82.koyeb.app/v1/health) • [GitHub](https://github.com/nirvagold/Rust-REVM)

</div>
