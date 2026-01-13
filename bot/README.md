# 🛡️ Ruster Shield Telegram Bot

Personal Telegram bot for honeypot detection using Ruster Shield API.

## Setup

### 1. Create Telegram Bot

1. Open Telegram and message [@BotFather](https://t.me/BotFather)
2. Send `/newbot`
3. Follow the instructions to create your bot
4. Copy the bot token (looks like `123456789:ABCdefGHIjklMNOpqrsTUVwxyz`)

### 2. Configure Environment

```bash
# Copy example env
cp .env.example .env

# Edit .env and add your token
TELEGRAM_BOT_TOKEN=your_bot_token_here
```

### 3. Install Dependencies

```bash
cd bot
pip install -r requirements.txt
```

### 4. Run Bot

```bash
python telegram_bot.py
```

## Commands

| Command | Description |
|---------|-------------|
| `/check <address>` | Analyze token (auto-detect chain) |
| `/check <address> <chain_id>` | Analyze on specific chain |
| `/chains` | Show supported chains |
| `/help` | Show help |

## Examples

```
/check 0xdAC17F958D2ee523a2206206994597C13D831ec7
/check 0x... 56
```

## Supported Chains

| Chain ID | Chain | Symbol |
|----------|-------|--------|
| 0 | Auto-Detect | - |
| 1 | Ethereum | ETH |
| 56 | BSC | BNB |
| 137 | Polygon | MATIC |
| 42161 | Arbitrum | ETH |
| 10 | Optimism | ETH |
| 43114 | Avalanche | AVAX |
| 8453 | Base | ETH |

## Response Example

```
🟢 USDT - Tether USD

✅ NOT A HONEYPOT

📊 Risk Score: 15/100 (SAFE)
🔗 Chain: Ethereum

💰 Market Data:
├ Price: $1.00
├ Liquidity: $125.5M
└ Volume 24h: $45.2M

💸 Tax Breakdown:
├ Buy Tax: 0.00%
├ Sell Tax: 0.00%
└ Total Loss: 0.00%

🧪 Simulation:
├ Buy: ✅
├ Sell: ✅
└ Latency: 450ms
```

## Running as Service (Linux)

Create systemd service:

```bash
sudo nano /etc/systemd/system/ruster-bot.service
```

```ini
[Unit]
Description=Ruster Shield Telegram Bot
After=network.target

[Service]
Type=simple
User=your_user
WorkingDirectory=/path/to/project/bot
ExecStart=/usr/bin/python3 telegram_bot.py
Restart=always
RestartSec=10
Environment=TELEGRAM_BOT_TOKEN=your_token

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl enable ruster-bot
sudo systemctl start ruster-bot
```
