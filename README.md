# Ruster REVM 🦀⚡

High-performance REVM-based token risk analyzer built in Rust. Pre-Execution Risk Scoring (PERS) engine that detects honeypots and malicious tokens before you trade.

## The PERS Algorithm

Ruster REVM implements the **Pre-Execution Risk Scoring (PERS)** algorithm:

```
R = Σ(wᵢ · sᵢ) for i=1 to n
```

Where:
- **s₁ (Simulation)**: Buy-Approve-Sell cycle success in isolated REVM environment
- **s₂ (Taxation)**: Deviation between theoretical and actual `amountOut`
- **s₃ (Liquidity)**: Pool depth relative to mempool swap size
- **s₄ (MEV Exposure)**: Sandwich attack vulnerability score
- **s₅ (Contract Risk)**: Proxy patterns, ownership, and code analysis

## Features

- **REVM Simulation**: Full EVM execution without on-chain transactions
- **Honeypot Detection**: Simulated Buy-Approve-Sell cycles catch 99%+ honeypots
- **Tax Analysis**: Precise buy/sell tax calculation via simulation
- **MEV Risk Scoring**: Identifies sandwich attack targets
- **Sub-50ms Latency**: Optimized for real-time trading decisions
- **REST API**: Production-ready API with batch processing

## Quick Start

### 1. Get RPC Access

Sign up for a free account at:
- [Alchemy](https://www.alchemy.com/) (recommended)
- [QuickNode](https://www.quicknode.com/)

### 2. Set Environment Variables

```powershell
# Windows PowerShell
$env:ETH_WSS_URL = "wss://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY"
```

```bash
# Linux/Mac
export ETH_WSS_URL="wss://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY"
```

### 3. Build & Run

```bash
# Run mempool analyzer
cargo run --release --bin ruster_revm

# Run REST API server
cargo run --release --bin ruster_api
```

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/analyze/token` | POST | Full PERS risk analysis |
| `/v1/honeypot/check` | POST | Quick honeypot detection |
| `/v1/analyze/batch` | POST | Batch analysis (up to 100 tokens) |
| `/v1/stats` | GET | Protection statistics |
| `/v1/health` | GET | Health check |

## Architecture

```
ruster_revm/
├── src/
│   ├── main.rs        # Mempool analyzer entry point
│   ├── bin/
│   │   └── ruster_api.rs  # REST API server
│   ├── analyzer.rs    # Core analysis orchestrator
│   ├── honeypot.rs    # REVM-based honeypot detection
│   ├── risk_score.rs  # PERS algorithm implementation
│   ├── simulator.rs   # REVM transaction simulator
│   └── types.rs       # Data structures
├── api/
│   └── openapi.yaml   # API specification
└── sdk/
    └── python/        # Python SDK
```

## Risk Levels

| Score | Level | Emoji | Action |
|-------|-------|-------|--------|
| 0-20 | SAFE | ✅ | Trade freely |
| 21-40 | LOW | 🟡 | Proceed with caution |
| 41-60 | MEDIUM | 🟠 | Review before trading |
| 61-80 | HIGH | 🔴 | Likely to lose funds |
| 81-100 | CRITICAL | 💀 | Do not trade |

## Performance

| Metric | Value |
|--------|-------|
| Honeypot Detection | ~15-30ms |
| Full PERS Analysis | ~30-50ms |
| Batch (100 tokens) | ~2-5s |
| Memory Usage | ~100MB |

## License

MIT
