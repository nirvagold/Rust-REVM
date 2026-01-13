#!/usr/bin/env python3
"""
🛡️ Ruster Shield Trading Bot (PRIVATE USE ONLY)
Full trading bot with honeypot detection + auto buy/sell

⚠️ WARNING: This bot handles REAL MONEY!

Commands:
  /check <token> - Analyze token with buy/sell buttons
  /buy <token> <amount_bnb> - Buy token (requires PIN)
  /sell <token> <percent> - Sell token (requires PIN)
  /balance - Check wallet balance
  /setpin <pin> - Set trading PIN
  /help - Show help
"""

import os
import asyncio
import aiohttp
import hashlib
from decimal import Decimal
from datetime import datetime, timedelta
from typing import Dict, Any
from dotenv import load_dotenv
from web3 import Web3
from web3.middleware import geth_poa_middleware
from eth_account import Account

load_dotenv()

# ============================================
# CONFIG
# ============================================
BOT_TOKEN = os.getenv("TELEGRAM_BOT_TOKEN", "")
PRIVATE_KEY = os.getenv("WALLET_PRIVATE_KEY", "")
OWNER_CHAT_ID = os.getenv("OWNER_CHAT_ID", "")
API_BASE = "https://yelling-patience-nirvagold-0a943e82.koyeb.app"

# PIN storage (in production, use encrypted storage)
stored_pin_hash = os.getenv("TRADING_PIN_HASH", "")

# Chain config (BSC default - cheapest gas)
CHAIN = {
    "rpc": "https://bsc-dataseed1.binance.org",
    "chain_id": 56,
    "symbol": "BNB",
    "router": "0x10ED43C718714eb63d5aA57B78B54704E256024E",
    "weth": "0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c",
    "explorer": "https://bscscan.com/tx/",
}

# Safety limits
MAX_BUY = Decimal("0.5")  # Max 0.5 BNB per trade
SLIPPAGE = 15  # 15%
GAS_LIMIT = 500000

# ABIs
ROUTER_ABI = [
    {"inputs":[{"name":"amountOutMin","type":"uint256"},{"name":"path","type":"address[]"},{"name":"to","type":"address"},{"name":"deadline","type":"uint256"}],"name":"swapExactETHForTokensSupportingFeeOnTransferTokens","outputs":[],"stateMutability":"payable","type":"function"},
    {"inputs":[{"name":"amountIn","type":"uint256"},{"name":"amountOutMin","type":"uint256"},{"name":"path","type":"address[]"},{"name":"to","type":"address"},{"name":"deadline","type":"uint256"}],"name":"swapExactTokensForETHSupportingFeeOnTransferTokens","outputs":[],"stateMutability":"nonpayable","type":"function"},
    {"inputs":[{"name":"amountIn","type":"uint256"},{"name":"path","type":"address[]"}],"name":"getAmountsOut","outputs":[{"name":"amounts","type":"uint256[]"}],"stateMutability":"view","type":"function"}
]

ERC20_ABI = [
    {"inputs":[{"name":"account","type":"address"}],"name":"balanceOf","outputs":[{"type":"uint256"}],"stateMutability":"view","type":"function"},
    {"inputs":[],"name":"decimals","outputs":[{"type":"uint8"}],"stateMutability":"view","type":"function"},
    {"inputs":[],"name":"symbol","outputs":[{"type":"string"}],"stateMutability":"view","type":"function"},
    {"inputs":[{"name":"spender","type":"address"},{"name":"amount","type":"uint256"}],"name":"approve","outputs":[{"type":"bool"}],"stateMutability":"nonpayable","type":"function"},
    {"inputs":[{"name":"owner","type":"address"},{"name":"spender","type":"address"}],"name":"allowance","outputs":[{"type":"uint256"}],"stateMutability":"view","type":"function"},
]

# State
pending_trades: Dict[int, Dict[str, Any]] = {}
pin_hash: str = stored_pin_hash

# ============================================
# HELPERS
# ============================================
def get_w3():
    w3 = Web3(Web3.HTTPProvider(CHAIN["rpc"]))
    w3.middleware_onion.inject(geth_poa_middleware, layer=0)
    return w3

def get_address():
    if not PRIVATE_KEY: return ""
    return Account.from_key(PRIVATE_KEY).address

def hash_pin(pin: str) -> str:
    return hashlib.sha256(pin.encode()).hexdigest()

def verify_pin(pin: str) -> bool:
    return pin_hash and hash_pin(pin) == pin_hash

async def send_msg(chat_id: int, text: str, buttons: dict = None):
    async with aiohttp.ClientSession() as s:
        payload = {"chat_id": chat_id, "text": text, "parse_mode": "Markdown", "disable_web_page_preview": True}
        if buttons: payload["reply_markup"] = buttons
        await s.post(f"https://api.telegram.org/bot{BOT_TOKEN}/sendMessage", json=payload)

# ============================================
# TRADING FUNCTIONS
# ============================================
async def execute_buy(token: str, amount_bnb: Decimal) -> dict:
    try:
        w3 = get_w3()
        addr = get_address()
        router = w3.eth.contract(address=Web3.to_checksum_address(CHAIN["router"]), abi=ROUTER_ABI)
        
        amount_wei = int(amount_bnb * Decimal(10**18))
        path = [Web3.to_checksum_address(CHAIN["weth"]), Web3.to_checksum_address(token)]
        
        amounts = router.functions.getAmountsOut(amount_wei, path).call()
        min_out = int(amounts[1] * (100 - SLIPPAGE) / 100)
        deadline = int((datetime.now() + timedelta(minutes=20)).timestamp())
        
        tx = router.functions.swapExactETHForTokensSupportingFeeOnTransferTokens(
            min_out, path, addr, deadline
        ).build_transaction({
            "from": addr, "value": amount_wei, "gas": GAS_LIMIT,
            "gasPrice": w3.eth.gas_price, "nonce": w3.eth.get_transaction_count(addr)
        })
        
        signed = w3.eth.account.sign_transaction(tx, PRIVATE_KEY)
        tx_hash = w3.eth.send_raw_transaction(signed.rawTransaction)
        receipt = w3.eth.wait_for_transaction_receipt(tx_hash, timeout=120)
        
        return {"success": receipt.status == 1, "tx": tx_hash.hex(), "url": f"{CHAIN['explorer']}{tx_hash.hex()}"}
    except Exception as e:
        return {"success": False, "error": str(e)}

async def execute_sell(token: str, percent: int) -> dict:
    try:
        w3 = get_w3()
        addr = get_address()
        token_addr = Web3.to_checksum_address(token)
        router_addr = Web3.to_checksum_address(CHAIN["router"])
        
        token_contract = w3.eth.contract(address=token_addr, abi=ERC20_ABI)
        balance = token_contract.functions.balanceOf(addr).call()
        sell_amount = int(balance * percent / 100)
        
        if sell_amount == 0:
            return {"success": False, "error": "No tokens to sell"}
        
        # Approve if needed
        allowance = token_contract.functions.allowance(addr, router_addr).call()
        if allowance < sell_amount:
            approve_tx = token_contract.functions.approve(router_addr, 2**256-1).build_transaction({
                "from": addr, "gas": 100000, "gasPrice": w3.eth.gas_price, "nonce": w3.eth.get_transaction_count(addr)
            })
            signed = w3.eth.account.sign_transaction(approve_tx, PRIVATE_KEY)
            w3.eth.send_raw_transaction(signed.rawTransaction)
            await asyncio.sleep(5)  # Wait for approval
        
        router = w3.eth.contract(address=router_addr, abi=ROUTER_ABI)
        path = [token_addr, Web3.to_checksum_address(CHAIN["weth"])]
        amounts = router.functions.getAmountsOut(sell_amount, path).call()
        min_out = int(amounts[1] * (100 - SLIPPAGE) / 100)
        deadline = int((datetime.now() + timedelta(minutes=20)).timestamp())
        
        tx = router.functions.swapExactTokensForETHSupportingFeeOnTransferTokens(
            sell_amount, min_out, path, addr, deadline
        ).build_transaction({
            "from": addr, "gas": GAS_LIMIT, "gasPrice": w3.eth.gas_price, "nonce": w3.eth.get_transaction_count(addr)
        })
        
        signed = w3.eth.account.sign_transaction(tx, PRIVATE_KEY)
        tx_hash = w3.eth.send_raw_transaction(signed.rawTransaction)
        receipt = w3.eth.wait_for_transaction_receipt(tx_hash, timeout=120)
        
        return {"success": receipt.status == 1, "tx": tx_hash.hex(), "url": f"{CHAIN['explorer']}{tx_hash.hex()}"}
    except Exception as e:
        return {"success": False, "error": str(e)}

# ============================================
# COMMAND HANDLERS
# ============================================
async def cmd_check(chat_id: int, token: str):
    await send_msg(chat_id, f"🔍 Checking `{token[:16]}...`")
    
    async with aiohttp.ClientSession() as s:
        async with s.post(f"{API_BASE}/v1/honeypot/check", json={"token_address": token, "chain_id": 56}, timeout=aiohttp.ClientTimeout(total=60)) as r:
            data = await r.json()
    
    if not data.get("success"):
        await send_msg(chat_id, f"❌ Error: {data.get('error', {}).get('message', 'Unknown')}")
        return
    
    d = data["data"]
    risk = d["risk_score"]
    safe = not d["is_honeypot"] and risk <= 40
    
    msg = f"""
{'🟢' if safe else '🔴'} *{d.get('token_symbol', '?')}* - {d.get('token_name', 'Unknown')}

{'✅ Safe' if safe else '⛔ RISKY'}

📊 Risk: `{risk}/100`
💸 Tax: Buy `{d['buy_tax_percent']:.1f}%` | Sell `{d['sell_tax_percent']:.1f}%`
🧪 Sim: {'✅' if d['buy_success'] else '❌'} Buy | {'✅' if d['sell_success'] else '❌'} Sell

`{token}`
"""
    
    buttons = None
    if safe:
        buttons = {"inline_keyboard": [
            [{"text": "🟢 Buy 0.01 BNB", "callback_data": f"buy:{token}:0.01"},
             {"text": "🟢 Buy 0.05 BNB", "callback_data": f"buy:{token}:0.05"}],
            [{"text": "🔴 Sell 50%", "callback_data": f"sell:{token}:50"},
             {"text": "🔴 Sell 100%", "callback_data": f"sell:{token}:100"}]
        ]}
    
    await send_msg(chat_id, msg, buttons)

async def cmd_balance(chat_id: int):
    addr = get_address()
    if not addr:
        await send_msg(chat_id, "❌ No wallet configured. Set WALLET_PRIVATE_KEY in .env")
        return
    
    w3 = get_w3()
    bal = w3.eth.get_balance(addr)
    await send_msg(chat_id, f"💰 *Balance*\n\n`{addr[:16]}...`\n`{Decimal(bal)/Decimal(10**18):.4f} BNB`")

async def cmd_setpin(chat_id: int, new_pin: str):
    global pin_hash
    if len(new_pin) < 4:
        await send_msg(chat_id, "❌ PIN must be at least 4 characters")
        return
    pin_hash = hash_pin(new_pin)
    await send_msg(chat_id, f"✅ PIN set! Hash: `{pin_hash[:16]}...`\n\n⚠️ Add to .env:\n`TRADING_PIN_HASH={pin_hash}`")

async def process_pin(chat_id: int, pin: str):
    if chat_id not in pending_trades:
        return
    
    if not verify_pin(pin):
        await send_msg(chat_id, "❌ Wrong PIN!")
        return
    
    trade = pending_trades.pop(chat_id)
    
    if trade["action"] == "buy":
        await send_msg(chat_id, "⏳ Executing buy...")
        result = await execute_buy(trade["token"], trade["amount"])
    else:
        await send_msg(chat_id, "⏳ Executing sell...")
        result = await execute_sell(trade["token"], trade["percent"])
    
    if result["success"]:
        await send_msg(chat_id, f"✅ *Success!*\n\n[View TX]({result['url']})")
    else:
        await send_msg(chat_id, f"❌ *Failed*\n\n`{result.get('error', 'Unknown error')}`")

# ============================================
# MAIN HANDLER
# ============================================
async def handle_update(update: dict):
    if "callback_query" in update:
        cb = update["callback_query"]
        chat_id = cb["message"]["chat"]["id"]
        data = cb["data"]
        
        # Verify owner
        if OWNER_CHAT_ID and str(chat_id) != OWNER_CHAT_ID:
            return
        
        parts = data.split(":")
        if parts[0] == "buy":
            token, amount = parts[1], Decimal(parts[2])
            pending_trades[chat_id] = {"action": "buy", "token": token, "amount": amount}
            await send_msg(chat_id, f"⚠️ *Confirm Buy*\n\nToken: `{token[:20]}...`\nAmount: `{amount} BNB`\n\n*Enter PIN:*")
        elif parts[0] == "sell":
            token, percent = parts[1], int(parts[2])
            pending_trades[chat_id] = {"action": "sell", "token": token, "percent": percent}
            await send_msg(chat_id, f"⚠️ *Confirm Sell*\n\nToken: `{token[:20]}...`\nSell: `{percent}%`\n\n*Enter PIN:*")
        return
    
    if "message" not in update:
        return
    
    msg = update["message"]
    chat_id = msg["chat"]["id"]
    text = msg.get("text", "")
    
    # Verify owner
    if OWNER_CHAT_ID and str(chat_id) != OWNER_CHAT_ID:
        await send_msg(chat_id, "⛔ Unauthorized")
        return
    
    # Check if PIN entry
    if chat_id in pending_trades and not text.startswith("/"):
        await process_pin(chat_id, text)
        return
    
    # Commands
    parts = text.split()
    cmd = parts[0].lower().split("@")[0] if parts else ""
    args = parts[1:] if len(parts) > 1 else []
    
    if cmd == "/start" or cmd == "/help":
        await send_msg(chat_id, """
🛡️ *Ruster Shield Trading Bot*

`/check <token>` - Analyze token
`/balance` - Check wallet
`/setpin <pin>` - Set trading PIN

_Use buttons to buy/sell after /check_
""")
    elif cmd == "/check" and args:
        await cmd_check(chat_id, args[0])
    elif cmd == "/balance":
        await cmd_balance(chat_id)
    elif cmd == "/setpin" and args:
        await cmd_setpin(chat_id, args[0])
    elif cmd.startswith("/"):
        await send_msg(chat_id, "❓ Unknown command. Use /help")

async def main():
    if not BOT_TOKEN:
        print("❌ Set TELEGRAM_BOT_TOKEN in .env")
        return
    
    print("🛡️ Trading Bot Starting...")
    print(f"   Wallet: {get_address()[:20]}..." if get_address() else "   ⚠️ No wallet configured")
    print(f"   PIN: {'Set ✅' if pin_hash else 'Not set ⚠️'}")
    
    offset = 0
    async with aiohttp.ClientSession() as session:
        while True:
            try:
                async with session.get(f"https://api.telegram.org/bot{BOT_TOKEN}/getUpdates", params={"offset": offset, "timeout": 30}) as r:
                    data = await r.json()
                    for u in data.get("result", []):
                        offset = u["update_id"] + 1
                        await handle_update(u)
            except Exception as e:
                print(f"❌ {e}")
                await asyncio.sleep(5)

if __name__ == "__main__":
    asyncio.run(main())
