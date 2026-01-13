#!/usr/bin/env python3
"""
🎯 Ruster Shield Sniper Bot
Auto-scanner + Honeypot checker + Telegram notifier

Architecture:
  DexScreener API (new pairs) 
       ↓
  Filter (age, liquidity, chain)
       ↓
  Ruster Shield API (honeypot check)
       ↓
  Telegram Alert + Buy Button
       ↓
  [Optional] Auto-execute via Web3

Features:
- Real-time DexScreener monitoring
- Auto honeypot detection via Koyeb API
- Telegram alerts with 1-click buy
- Configurable filters (min liquidity, max age, chains)
- Rate limiting to avoid API bans

⚠️ PRIVATE USE ONLY - Contains trading logic
"""

import os
import asyncio
import aiohttp
import time
from datetime import datetime, timedelta
from decimal import Decimal
from typing import Optional, Dict, List, Set
from dataclasses import dataclass
from dotenv import load_dotenv

load_dotenv()

# ============================================
# CONFIGURATION
# ============================================
TELEGRAM_BOT_TOKEN = os.getenv("TELEGRAM_BOT_TOKEN", "")
TELEGRAM_CHAT_ID = os.getenv("OWNER_CHAT_ID", "")  # Your personal chat ID

# APIs
RUSTER_API = "https://yelling-patience-nirvagold-0a943e82.koyeb.app"
DEXSCREENER_API = "https://api.dexscreener.com/latest/dex"

# Scanner filters
SCAN_CONFIG = {
    # Chains to monitor (chain_id: name)
    "chains": {
        "bsc": 56,
        "ethereum": 1,
        "base": 8453,
        "arbitrum": 42161,
    },
    
    # Pair age filter
    "max_pair_age_minutes": 30,      # Only pairs < 30 min old
    "min_pair_age_minutes": 2,       # Skip very new (might be rug setup)
    
    # Liquidity filter
    "min_liquidity_usd": 5000,       # Min $5k liquidity
    "max_liquidity_usd": 500000,     # Max $500k (skip established tokens)
    
    # Volume filter
    "min_volume_24h": 1000,          # Min $1k volume
    
    # Risk filter (from Ruster Shield)
    "max_risk_score": 40,            # Only SAFE or LOW risk
    "require_buy_success": True,     # Must pass buy simulation
    "require_sell_success": True,    # Must pass sell simulation
    "max_total_tax": 15,             # Max 15% total tax
    
    # Scan interval
    "scan_interval_seconds": 30,     # Check every 30s
    
    # Auto-buy settings (DANGEROUS!)
    "auto_buy_enabled": False,       # Set True to enable auto-buy
    "auto_buy_amount_bnb": "0.01",   # Amount per trade
    "auto_buy_max_daily": 5,         # Max 5 auto-buys per day
}

# Track seen pairs to avoid duplicates
seen_pairs: Set[str] = set()
daily_buys: int = 0
last_reset: datetime = datetime.now()

@dataclass
class NewPair:
    """Represents a newly discovered trading pair"""
    chain: str
    chain_id: int
    pair_address: str
    token_address: str
    token_name: str
    token_symbol: str
    price_usd: float
    liquidity_usd: float
    volume_24h: float
    pair_age_minutes: float
    dex_id: str

# ============================================
# DEXSCREENER SCANNER
# ============================================
async def fetch_new_pairs(session: aiohttp.ClientSession, chain: str) -> List[NewPair]:
    """Fetch latest pairs from DexScreener for a chain"""
    try:
        # DexScreener latest pairs endpoint
        url = f"https://api.dexscreener.com/latest/dex/pairs/{chain}"
        
        async with session.get(url, timeout=aiohttp.ClientTimeout(total=10)) as resp:
            if resp.status != 200:
                return []
            
            data = await resp.json()
            pairs = data.get("pairs", [])
            
            new_pairs = []
            now = datetime.now()
            
            for p in pairs:
                # Skip if already seen
                pair_key = f"{p.get('chainId')}:{p.get('pairAddress')}"
                if pair_key in seen_pairs:
                    continue
                
                # Parse pair age
                created_at = p.get("pairCreatedAt")
                if not created_at:
                    continue
                
                try:
                    pair_time = datetime.fromtimestamp(created_at / 1000)
                    age_minutes = (now - pair_time).total_seconds() / 60
                except:
                    continue
                
                # Apply filters
                liquidity = p.get("liquidity", {}).get("usd", 0) or 0
                volume = p.get("volume", {}).get("h24", 0) or 0
                
                cfg = SCAN_CONFIG
                if age_minutes > cfg["max_pair_age_minutes"]:
                    continue
                if age_minutes < cfg["min_pair_age_minutes"]:
                    continue
                if liquidity < cfg["min_liquidity_usd"]:
                    continue
                if liquidity > cfg["max_liquidity_usd"]:
                    continue
                if volume < cfg["min_volume_24h"]:
                    continue
                
                # Get token info
                base_token = p.get("baseToken", {})
                token_address = base_token.get("address", "")
                
                if not token_address:
                    continue
                
                new_pairs.append(NewPair(
                    chain=chain,
                    chain_id=cfg["chains"].get(chain, 0),
                    pair_address=p.get("pairAddress", ""),
                    token_address=token_address,
                    token_name=base_token.get("name", "Unknown"),
                    token_symbol=base_token.get("symbol", "???"),
                    price_usd=float(p.get("priceUsd", 0) or 0),
                    liquidity_usd=liquidity,
                    volume_24h=volume,
                    pair_age_minutes=age_minutes,
                    dex_id=p.get("dexId", "unknown"),
                ))
                
                # Mark as seen
                seen_pairs.add(pair_key)
            
            return new_pairs
            
    except Exception as e:
        print(f"❌ DexScreener error ({chain}): {e}")
        return []

# ============================================
# RUSTER SHIELD INTEGRATION
# ============================================
async def check_honeypot(session: aiohttp.ClientSession, token: str, chain_id: int) -> Optional[dict]:
    """Check token via Ruster Shield API"""
    try:
        async with session.post(
            f"{RUSTER_API}/v1/honeypot/check",
            json={
                "token_address": token,
                "chain_id": chain_id,
                "test_amount_eth": "0.1"
            },
            timeout=aiohttp.ClientTimeout(total=60)
        ) as resp:
            data = await resp.json()
            if data.get("success"):
                return data.get("data")
            return None
    except Exception as e:
        print(f"❌ Ruster API error: {e}")
        return None

def passes_safety_check(result: dict) -> tuple[bool, str]:
    """Check if token passes our safety criteria"""
    cfg = SCAN_CONFIG
    
    # Check honeypot
    if result.get("is_honeypot"):
        return False, "🍯 Honeypot detected"
    
    # Check risk score
    risk = result.get("risk_score", 100)
    if risk > cfg["max_risk_score"]:
        return False, f"⚠️ Risk too high: {risk}/100"
    
    # Check simulation success
    if cfg["require_buy_success"] and not result.get("buy_success"):
        return False, "❌ Buy simulation failed"
    
    if cfg["require_sell_success"] and not result.get("sell_success"):
        return False, "❌ Sell simulation failed"
    
    # Check tax
    buy_tax = result.get("buy_tax_percent", 0)
    sell_tax = result.get("sell_tax_percent", 0)
    total_tax = buy_tax + sell_tax
    
    if total_tax > cfg["max_total_tax"]:
        return False, f"💸 Tax too high: {total_tax:.1f}%"
    
    return True, "✅ Passed all checks"

# ============================================
# TELEGRAM NOTIFICATIONS
# ============================================
async def send_telegram(text: str, reply_markup: dict = None):
    """Send message to Telegram"""
    if not TELEGRAM_BOT_TOKEN or not TELEGRAM_CHAT_ID:
        print(f"📱 [Telegram disabled] {text[:100]}...")
        return
    
    async with aiohttp.ClientSession() as session:
        payload = {
            "chat_id": TELEGRAM_CHAT_ID,
            "text": text,
            "parse_mode": "Markdown",
            "disable_web_page_preview": True
        }
        if reply_markup:
            payload["reply_markup"] = reply_markup
        
        try:
            await session.post(
                f"https://api.telegram.org/bot{TELEGRAM_BOT_TOKEN}/sendMessage",
                json=payload
            )
        except Exception as e:
            print(f"❌ Telegram error: {e}")

def format_alert(pair: NewPair, result: dict, passed: bool, reason: str) -> str:
    """Format alert message"""
    risk = result.get("risk_score", 0)
    emoji = "🟢" if risk <= 20 else "🔵" if risk <= 40 else "🟡" if risk <= 60 else "🟠" if risk <= 80 else "🔴"
    
    status = "✅ *SAFE TO TRADE*" if passed else f"⛔ *REJECTED*\n_{reason}_"
    
    msg = f"""
🎯 *NEW PAIR DETECTED*

{emoji} *{pair.token_symbol}* - {pair.token_name}
{status}

📊 *Analysis:*
├ Risk Score: `{risk}/100`
├ Buy Tax: `{result.get('buy_tax_percent', 0):.1f}%`
├ Sell Tax: `{result.get('sell_tax_percent', 0):.1f}%`
├ Buy Sim: {'✅' if result.get('buy_success') else '❌'}
└ Sell Sim: {'✅' if result.get('sell_success') else '❌'}

💰 *Market:*
├ Price: `${pair.price_usd:.8f}`
├ Liquidity: `${pair.liquidity_usd:,.0f}`
├ Volume 24h: `${pair.volume_24h:,.0f}`
└ Age: `{pair.pair_age_minutes:.1f} min`

🔗 *Links:*
├ Chain: {pair.chain.upper()} ({pair.dex_id})
├ [DexScreener](https://dexscreener.com/{pair.chain}/{pair.pair_address})
└ [Token](https://dexscreener.com/{pair.chain}/{pair.token_address})

`{pair.token_address}`
"""
    return msg

def get_buy_buttons(pair: NewPair) -> dict:
    """Generate inline keyboard for buy actions"""
    return {
        "inline_keyboard": [
            [
                {"text": "🟢 Buy 0.01", "callback_data": f"snipe:buy:{pair.token_address}:0.01:{pair.chain_id}"},
                {"text": "🟢 Buy 0.05", "callback_data": f"snipe:buy:{pair.token_address}:0.05:{pair.chain_id}"},
                {"text": "🟢 Buy 0.1", "callback_data": f"snipe:buy:{pair.token_address}:0.1:{pair.chain_id}"},
            ],
            [
                {"text": "📊 Re-check", "callback_data": f"snipe:check:{pair.token_address}:{pair.chain_id}"},
                {"text": "🔗 DexScreener", "url": f"https://dexscreener.com/{pair.chain}/{pair.pair_address}"},
            ]
        ]
    }

# ============================================
# MAIN SCANNER LOOP
# ============================================
async def scan_chain(session: aiohttp.ClientSession, chain: str):
    """Scan a single chain for new pairs"""
    pairs = await fetch_new_pairs(session, chain)
    
    for pair in pairs:
        print(f"🔍 Checking {pair.token_symbol} on {chain}...")
        
        # Check via Ruster Shield
        result = await check_honeypot(session, pair.token_address, pair.chain_id)
        
        if not result:
            print(f"   ⚠️ API check failed, skipping")
            continue
        
        # Safety check
        passed, reason = passes_safety_check(result)
        
        # Format and send alert
        msg = format_alert(pair, result, passed, reason)
        
        if passed:
            # Safe token - send with buy buttons
            await send_telegram(msg, get_buy_buttons(pair))
            print(f"   ✅ SAFE - Alert sent!")
            
            # Auto-buy if enabled
            global daily_buys
            if SCAN_CONFIG["auto_buy_enabled"] and daily_buys < SCAN_CONFIG["auto_buy_max_daily"]:
                print(f"   🤖 Auto-buy triggered!")
                # TODO: Integrate with trading_bot.py execute_buy()
                daily_buys += 1
        else:
            # Risky token - send without buy buttons (info only)
            await send_telegram(msg)
            print(f"   ⛔ REJECTED: {reason}")
        
        # Rate limit between checks
        await asyncio.sleep(2)

async def scanner_loop():
    """Main scanner loop"""
    global daily_buys, last_reset
    
    print("🎯 Ruster Shield Sniper Bot Starting...")
    print(f"   API: {RUSTER_API}")
    print(f"   Chains: {list(SCAN_CONFIG['chains'].keys())}")
    print(f"   Max Risk: {SCAN_CONFIG['max_risk_score']}")
    print(f"   Auto-buy: {'ENABLED ⚠️' if SCAN_CONFIG['auto_buy_enabled'] else 'Disabled'}")
    print()
    
    await send_telegram("🎯 *Sniper Bot Started*\n\nMonitoring for new pairs...")
    
    async with aiohttp.ClientSession() as session:
        while True:
            try:
                # Reset daily counter
                if datetime.now().date() > last_reset.date():
                    daily_buys = 0
                    last_reset = datetime.now()
                
                # Scan all chains
                for chain in SCAN_CONFIG["chains"].keys():
                    await scan_chain(session, chain)
                    await asyncio.sleep(1)  # Rate limit between chains
                
                # Wait before next scan
                await asyncio.sleep(SCAN_CONFIG["scan_interval_seconds"])
                
            except Exception as e:
                print(f"❌ Scanner error: {e}")
                await asyncio.sleep(10)

# ============================================
# ENTRY POINT
# ============================================
if __name__ == "__main__":
    if not TELEGRAM_BOT_TOKEN:
        print("⚠️ TELEGRAM_BOT_TOKEN not set - alerts will be printed only")
    if not TELEGRAM_CHAT_ID:
        print("⚠️ OWNER_CHAT_ID not set - alerts will be printed only")
    
    asyncio.run(scanner_loop())
