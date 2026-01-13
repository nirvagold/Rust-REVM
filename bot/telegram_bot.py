#!/usr/bin/env python3
"""
🛡️ Ruster Shield Telegram Bot
Personal honeypot detection bot using Ruster Shield API

Usage:
  /check <token_address> - Check token for honeypot
  /check <token_address> <chain_id> - Check on specific chain
  /chains - Show supported chains
  /help - Show help

Setup:
  1. Create bot via @BotFather on Telegram
  2. Set TELEGRAM_BOT_TOKEN in .env
  3. Run: python bot/telegram_bot.py
"""

import os
import asyncio
import aiohttp
from datetime import datetime
from dotenv import load_dotenv

# Load environment variables
load_dotenv()

# Telegram Bot Token (get from @BotFather)
BOT_TOKEN = os.getenv("TELEGRAM_BOT_TOKEN", "")

# Ruster Shield API
API_BASE = "https://yelling-patience-nirvagold-0a943e82.koyeb.app"

# Supported chains
CHAINS = {
    0: ("🔍 Auto-Detect", "auto"),
    1: ("🔷 Ethereum", "ETH"),
    56: ("🟡 BSC", "BNB"),
    137: ("🟣 Polygon", "MATIC"),
    42161: ("🔵 Arbitrum", "ETH"),
    10: ("🔴 Optimism", "ETH"),
    43114: ("🔺 Avalanche", "AVAX"),
    8453: ("🔵 Base", "ETH"),
}

def get_risk_emoji(score: int) -> str:
    """Get emoji based on risk score"""
    if score <= 20:
        return "🟢"
    elif score <= 40:
        return "🔵"
    elif score <= 60:
        return "🟡"
    elif score <= 80:
        return "🟠"
    return "🔴"

def get_risk_level(score: int) -> str:
    """Get risk level text"""
    if score <= 20:
        return "SAFE"
    elif score <= 40:
        return "LOW"
    elif score <= 60:
        return "MEDIUM"
    elif score <= 80:
        return "HIGH"
    return "CRITICAL"

def format_usd(value: float | None) -> str:
    """Format USD value"""
    if value is None:
        return "N/A"
    if value >= 1_000_000:
        return f"${value/1_000_000:.2f}M"
    if value >= 1_000:
        return f"${value/1_000:.2f}K"
    return f"${value:.2f}"

def format_price(price: str | None) -> str:
    """Format price"""
    if not price:
        return "N/A"
    try:
        num = float(price)
        if num < 0.00001:
            return f"${num:.2e}"
        if num < 1:
            return f"${num:.6f}"
        return f"${num:.2f}"
    except:
        return "N/A"

async def check_honeypot(token_address: str, chain_id: int = 0) -> dict:
    """Call Ruster Shield API to check token"""
    async with aiohttp.ClientSession() as session:
        try:
            async with session.post(
                f"{API_BASE}/v1/honeypot/check",
                json={
                    "token_address": token_address,
                    "chain_id": chain_id,
                    "test_amount_eth": "0.1"
                },
                timeout=aiohttp.ClientTimeout(total=60)
            ) as resp:
                return await resp.json()
        except asyncio.TimeoutError:
            return {"success": False, "error": {"message": "Request timeout (60s)"}}
        except Exception as e:
            return {"success": False, "error": {"message": str(e)}}

def format_result(data: dict) -> str:
    """Format API response as Telegram message"""
    if not data.get("success"):
        error = data.get("error", {}).get("message", "Unknown error")
        return f"❌ *Analysis Failed*\n\n`{error}`"
    
    d = data["data"]
    risk_emoji = get_risk_emoji(d["risk_score"])
    risk_level = get_risk_level(d["risk_score"])
    
    # Honeypot status
    if d["is_honeypot"]:
        hp_status = "🍯 *HONEYPOT DETECTED*"
    else:
        hp_status = "✅ *NOT A HONEYPOT*"
    
    # Build message
    msg = f"""
{risk_emoji} *{d.get('token_symbol', 'Unknown')}* - {d.get('token_name', 'Unknown Token')}

{hp_status}

📊 *Risk Score:* `{d['risk_score']}/100` ({risk_level})
🔗 *Chain:* {d['chain_name']}
"""

    # Market data (if available)
    if d.get("price_usd") or d.get("liquidity_usd"):
        msg += f"""
💰 *Market Data:*
├ Price: `{format_price(d.get('price_usd'))}`
├ Liquidity: `{format_usd(d.get('liquidity_usd'))}`
└ Volume 24h: `{format_usd(d.get('volume_24h_usd'))}`
"""

    # Tax info
    buy_ok = d.get("buy_success", False)
    sell_ok = d.get("sell_success", False)
    
    if buy_ok or sell_ok:
        msg += f"""
💸 *Tax Breakdown:*
├ Buy Tax: `{d['buy_tax_percent']:.2f}%`
├ Sell Tax: `{d['sell_tax_percent']:.2f}%`
└ Total Loss: `{d['total_loss_percent']:.2f}%`
"""
    else:
        msg += f"""
⚠️ *Tax:* _Unverified (No V2 liquidity)_
"""

    # Simulation results
    msg += f"""
🧪 *Simulation:*
├ Buy: {'✅' if buy_ok else '❌'}
├ Sell: {'✅' if sell_ok else '❌'}
└ Latency: `{d.get('simulation_latency_ms', 0)}ms`
"""

    # Reason
    msg += f"""
📝 *Reason:* _{d.get('reason', 'N/A')}_
"""

    # DexScreener link
    if d.get("pair_address"):
        chain_slug = d['chain_name'].lower().replace(" ", "")
        msg += f"""
🔗 [View on DexScreener](https://dexscreener.com/{chain_slug}/{d['pair_address']})
"""

    # Token address
    msg += f"""
📋 `{d['token_address']}`
"""

    return msg

async def handle_update(update: dict):
    """Handle incoming Telegram update"""
    if "message" not in update:
        return
    
    message = update["message"]
    chat_id = message["chat"]["id"]
    text = message.get("text", "")
    
    if not text.startswith("/"):
        return
    
    parts = text.split()
    command = parts[0].lower().split("@")[0]  # Remove @botname suffix
    
    if command == "/start" or command == "/help":
        await send_message(chat_id, """
🛡️ *Ruster Shield Bot*
_Pre-Execution Token Risk Analyzer_

*Commands:*
`/check <address>` - Analyze token (auto-detect chain)
`/check <address> <chain_id>` - Analyze on specific chain
`/chains` - Show supported chains

*Example:*
`/check 0xdAC17F958D2ee523a2206206994597C13D831ec7`
`/check 0x... 56` (BSC)

_Powered by REVM Simulation_
""")
    
    elif command == "/chains":
        chains_text = "*Supported Chains:*\n\n"
        for cid, (name, symbol) in CHAINS.items():
            chains_text += f"`{cid}` - {name} ({symbol})\n"
        chains_text += "\n_Use chain\\_id 0 for auto-detection_"
        await send_message(chat_id, chains_text)
    
    elif command == "/check":
        if len(parts) < 2:
            await send_message(chat_id, "❌ Usage: `/check <token_address> [chain_id]`")
            return
        
        token_address = parts[1]
        chain_id = int(parts[2]) if len(parts) > 2 else 0
        
        # Validate address
        if not token_address.startswith("0x") or len(token_address) != 42:
            await send_message(chat_id, "❌ Invalid token address. Must be 0x... (42 chars)")
            return
        
        # Send "analyzing" message
        await send_message(chat_id, f"🔍 Analyzing `{token_address[:10]}...{token_address[-6:]}`\n\n_Please wait..._")
        
        # Call API
        result = await check_honeypot(token_address, chain_id)
        
        # Send result
        await send_message(chat_id, format_result(result))
    
    else:
        await send_message(chat_id, "❓ Unknown command. Use /help for available commands.")

async def send_message(chat_id: int, text: str):
    """Send message to Telegram chat"""
    async with aiohttp.ClientSession() as session:
        await session.post(
            f"https://api.telegram.org/bot{BOT_TOKEN}/sendMessage",
            json={
                "chat_id": chat_id,
                "text": text,
                "parse_mode": "Markdown",
                "disable_web_page_preview": True
            }
        )

async def main():
    """Main polling loop"""
    if not BOT_TOKEN:
        print("❌ TELEGRAM_BOT_TOKEN not set!")
        print("   1. Create bot via @BotFather")
        print("   2. Add TELEGRAM_BOT_TOKEN to .env file")
        return
    
    print("🛡️ Ruster Shield Telegram Bot starting...")
    print(f"   API: {API_BASE}")
    
    offset = 0
    async with aiohttp.ClientSession() as session:
        while True:
            try:
                async with session.get(
                    f"https://api.telegram.org/bot{BOT_TOKEN}/getUpdates",
                    params={"offset": offset, "timeout": 30}
                ) as resp:
                    data = await resp.json()
                    
                    if data.get("ok") and data.get("result"):
                        for update in data["result"]:
                            offset = update["update_id"] + 1
                            await handle_update(update)
            
            except Exception as e:
                print(f"❌ Error: {e}")
                await asyncio.sleep(5)

if __name__ == "__main__":
    asyncio.run(main())
