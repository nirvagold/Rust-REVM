# 🛡️ Ruster Shield Bot Suite

Personal trading bots powered by Ruster Shield API.

## Bots Available

| Bot | Purpose | Risk Level |
|-----|---------|------------|
| `telegram_bot.py` | Manual token checking | 🟢 Safe |
| `sniper_bot.py` | Auto-scanner + alerts | 🟡 Medium |
| `trading_bot.py` | Full trading (buy/sell) | 🔴 High |

## Quick Start

### 1. Setup Environment

```bash
cp .env.example .env
# Edit .env with your values
```

### 2. Install Dependencies

```bash
cd bot
pip install -r requirements.txt
```

### 3. Get Telegram Credentials

1. Create bot via [@BotFather](https://t.me/BotFather) → get `TELEGRAM_BOT_TOKEN`
2. Message [@userinfobot](https://t.me/userinfobot) → get `OWNER_CHAT_ID`

## Bot Details

### 📱 Telegram Bot (`telegram_bot.py`)

Simple manual checker.

```bash
python telegram_bot.py
```

Commands:
- `/check <address>` - Check token
- `/chains` - Show supported chains
- `/help` - Help

---

### 🎯 Sniper Bot (`sniper_bot.py`)

Auto-scans DexScreener for new pairs, checks via Ruster Shield API, sends alerts.

```bash
python sniper_bot.py
```

Features:
- Monitors BSC, ETH, Base, Arbitrum
- Filters by age, liquidity, volume
- Auto honeypot check via API
- Telegram alerts with buy buttons
- Optional auto-buy (disabled by default)

Configuration (in `sniper_bot.py`):
```python
SCAN_CONFIG = {
    "max_pair_age_minutes": 30,
    "min_liquidity_usd": 5000,
    "max_risk_score": 40,
    "auto_buy_enabled": False,  # ⚠️ DANGEROUS
}
```

---

### 💰 Trading Bot (`trading_bot.py`)

Full trading with PIN protection.

```bash
python trading_bot.py
```

Commands:
- `/check <address>` - Check + buy buttons
- `/buy <address> <amount>` - Buy (requires PIN)
- `/sell <address> <percent>` - Sell (requires PIN)
- `/balance` - Check wallet
- `/setpin <pin>` - Set trading PIN

⚠️ **REQUIRES:**
- `WALLET_PRIVATE_KEY` in .env
- `TRADING_PIN` set via `/setpin`

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Your Computer                        │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐  │
│  │ Sniper Bot  │  │ Trading Bot │  │  Telegram Bot   │  │
│  │ (scanner)   │  │ (executor)  │  │  (manual)       │  │
│  └──────┬──────┘  └──────┬──────┘  └────────┬────────┘  │
│         │                │                   │          │
│         └────────────────┼───────────────────┘          │
│                          │                              │
└──────────────────────────┼──────────────────────────────┘
                           │ HTTP
                           ▼
              ┌────────────────────────┐
              │   Koyeb (Ruster API)   │
              │   /v1/honeypot/check   │
              └────────────┬───────────┘
                           │
              ┌────────────┴───────────┐
              │                        │
              ▼                        ▼
        ┌──────────┐            ┌──────────────┐
        │ Alchemy  │            │ DexScreener  │
        │  (RPC)   │            │   (prices)   │
        └──────────┘            └──────────────┘
```

## Safety Notes

1. **Never share your private key**
2. **Start with small amounts** (0.01 BNB)
3. **Test on BSC first** (cheapest gas)
4. **Keep auto-buy disabled** until you trust the system
5. **Monitor your wallet** regularly

## Troubleshooting

### "No V2 liquidity"
Token trades on Uniswap V3 or unsupported DEX. Skip it.

### "Risk too high"
Token failed safety checks. Don't buy.

### "API timeout"
Koyeb server busy. Wait and retry.
