#!/usr/bin/env python3
"""
🧪 Test Solana Token Analysis
Test token dari DexScreener di Solana chain
"""

import asyncio
import aiohttp
import os
from datetime import datetime
from dotenv import load_dotenv

load_dotenv()

ALCHEMY_API_KEY = os.getenv("ALCHEMY_API_KEY", "")
SOLANA_RPC = f"https://solana-mainnet.g.alchemy.com/v2/{ALCHEMY_API_KEY}"

# Token dari user: Groyper (pump.fun)
TOKEN_ADDRESS = "5JvNvucsQUgXWhyVTUPNbmg8CVeyt97ZwnumDhT6pump"

async def get_dexscreener_data(token_address: str):
    """Get token data from DexScreener"""
    print(f"\n📊 Fetching DexScreener data for {token_address}...")
    
    async with aiohttp.ClientSession() as session:
        url = f"https://api.dexscreener.com/latest/dex/tokens/{token_address}"
        async with session.get(url) as resp:
            if resp.status == 200:
                data = await resp.json()
                pairs = data.get("pairs", [])
                
                if not pairs:
                    print("   ❌ No pairs found on DexScreener")
                    return None
                    
                # Get best pair (highest liquidity)
                best_pair = max(pairs, key=lambda p: p.get("liquidity", {}).get("usd", 0))
                
                print(f"   ✅ Found {len(pairs)} pairs")
                print(f"\n   📈 Best Pair Info:")
                print(f"      Chain: {best_pair.get('chainId', 'N/A')}")
                print(f"      DEX: {best_pair.get('dexId', 'N/A')}")
                print(f"      Name: {best_pair.get('baseToken', {}).get('name', 'N/A')}")
                print(f"      Symbol: {best_pair.get('baseToken', {}).get('symbol', 'N/A')}")
                print(f"      Price USD: ${float(best_pair.get('priceUsd', 0)):.8f}")
                print(f"      Liquidity: ${best_pair.get('liquidity', {}).get('usd', 0):,.2f}")
                print(f"      Volume 24h: ${best_pair.get('volume', {}).get('h24', 0):,.2f}")
                print(f"      Market Cap: ${best_pair.get('marketCap', 0):,.2f}")
                
                # Trading activity
                txns = best_pair.get("txns", {})
                h1 = txns.get("h1", {})
                print(f"\n   📊 Trading Activity (1h):")
                print(f"      Buys: {h1.get('buys', 0)}")
                print(f"      Sells: {h1.get('sells', 0)}")
                
                # Price change
                price_change = best_pair.get("priceChange", {})
                print(f"\n   📈 Price Change:")
                print(f"      5m: {price_change.get('m5', 0):.2f}%")
                print(f"      1h: {price_change.get('h1', 0):.2f}%")
                print(f"      24h: {price_change.get('h24', 0):.2f}%")
                
                # Social info
                info = best_pair.get("info", {})
                if info:
                    print(f"\n   🌐 Social Info:")
                    websites = info.get("websites", [])
                    socials = info.get("socials", [])
                    if websites:
                        print(f"      Website: {websites[0].get('url', 'N/A')}")
                    if socials:
                        for social in socials:
                            print(f"      {social.get('type', 'N/A').title()}: {social.get('url', 'N/A')}")
                
                return best_pair
            else:
                print(f"   ❌ DexScreener error: {resp.status}")
                return None

async def get_solana_token_info(token_address: str):
    """Get token info from Solana RPC (Alchemy)"""
    print(f"\n🔗 Fetching Solana RPC data...")
    
    async with aiohttp.ClientSession() as session:
        # Get token supply
        payload = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getTokenSupply",
            "params": [token_address]
        }
        
        async with session.post(SOLANA_RPC, json=payload) as resp:
            if resp.status == 200:
                data = await resp.json()
                result = data.get("result", {}).get("value", {})
                
                if result:
                    amount = int(result.get("amount", 0))
                    decimals = result.get("decimals", 9)
                    ui_amount = result.get("uiAmount", 0)
                    
                    print(f"   ✅ Token Supply Info:")
                    print(f"      Total Supply: {ui_amount:,.0f}")
                    print(f"      Decimals: {decimals}")
                    print(f"      Raw Amount: {amount}")
                else:
                    print(f"   ⚠️ No supply info found")
            else:
                print(f"   ❌ RPC error: {resp.status}")
                
        # Get largest token accounts (whale detection)
        payload = {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "getTokenLargestAccounts",
            "params": [token_address]
        }
        
        async with session.post(SOLANA_RPC, json=payload) as resp:
            if resp.status == 200:
                data = await resp.json()
                accounts = data.get("result", {}).get("value", [])
                
                if accounts:
                    print(f"\n   🐋 Top Token Holders:")
                    total_supply = sum(int(a.get("amount", 0)) for a in accounts)
                    
                    for i, acc in enumerate(accounts[:5], 1):
                        amount = int(acc.get("amount", 0))
                        ui_amount = acc.get("uiAmount", 0)
                        address = acc.get("address", "")[:20] + "..."
                        pct = (amount / total_supply * 100) if total_supply > 0 else 0
                        print(f"      #{i}: {ui_amount:,.0f} ({pct:.1f}%) - {address}")
                else:
                    print(f"   ⚠️ No holder info found")
            else:
                print(f"   ❌ RPC error: {resp.status}")

def analyze_risk(dex_data: dict) -> dict:
    """Analyze token risk based on available data"""
    print(f"\n🔍 Risk Analysis:")
    
    risk_score = 50  # Start neutral
    risk_factors = []
    
    if not dex_data:
        return {"score": 100, "level": "UNKNOWN", "factors": ["No data available"]}
    
    # 1. Liquidity check
    liquidity = dex_data.get("liquidity", {}).get("usd", 0)
    if liquidity < 1000:
        risk_score += 30
        risk_factors.append(f"⚠️ Very low liquidity: ${liquidity:,.0f}")
    elif liquidity < 10000:
        risk_score += 15
        risk_factors.append(f"⚠️ Low liquidity: ${liquidity:,.0f}")
    elif liquidity > 100000:
        risk_score -= 15
        risk_factors.append(f"✅ Good liquidity: ${liquidity:,.0f}")
    
    # 2. Age check (pair created at)
    created_at = dex_data.get("pairCreatedAt", 0)
    if created_at:
        age_hours = (datetime.now().timestamp() * 1000 - created_at) / (1000 * 60 * 60)
        if age_hours < 1:
            risk_score += 25
            risk_factors.append(f"⚠️ Very new token: {age_hours:.1f} hours old")
        elif age_hours < 24:
            risk_score += 10
            risk_factors.append(f"⚠️ New token: {age_hours:.1f} hours old")
        elif age_hours > 168:  # 7 days
            risk_score -= 10
            risk_factors.append(f"✅ Established token: {age_hours/24:.0f} days old")
    
    # 3. Trading activity
    txns = dex_data.get("txns", {}).get("h1", {})
    buys = txns.get("buys", 0)
    sells = txns.get("sells", 0)
    
    if buys + sells < 10:
        risk_score += 15
        risk_factors.append(f"⚠️ Low trading activity: {buys+sells} txns/hour")
    elif buys + sells > 100:
        risk_score -= 10
        risk_factors.append(f"✅ Active trading: {buys+sells} txns/hour")
    
    # Buy/sell ratio
    if sells > 0 and buys > 0:
        ratio = buys / sells
        if ratio < 0.5:
            risk_score += 15
            risk_factors.append(f"⚠️ More sells than buys: {ratio:.2f} ratio")
        elif ratio > 2:
            risk_score -= 5
            risk_factors.append(f"✅ More buys than sells: {ratio:.2f} ratio")
    
    # 4. Price volatility
    price_change = dex_data.get("priceChange", {})
    h1_change = abs(price_change.get("h1", 0))
    if h1_change > 50:
        risk_score += 15
        risk_factors.append(f"⚠️ High volatility: {h1_change:.0f}% in 1h")
    
    # 5. DEX check
    dex_id = dex_data.get("dexId", "")
    if "pump" in dex_id.lower():
        risk_score += 10
        risk_factors.append(f"⚠️ Pump.fun token (high risk category)")
    
    # 6. Social presence
    info = dex_data.get("info", {})
    if info.get("socials"):
        risk_score -= 5
        risk_factors.append(f"✅ Has social media presence")
    else:
        risk_score += 5
        risk_factors.append(f"⚠️ No social media found")
    
    # Clamp score
    risk_score = max(0, min(100, risk_score))
    
    # Determine level
    if risk_score <= 30:
        level = "LOW"
        emoji = "🟢"
    elif risk_score <= 50:
        level = "MEDIUM"
        emoji = "🟡"
    elif risk_score <= 70:
        level = "HIGH"
        emoji = "🟠"
    else:
        level = "CRITICAL"
        emoji = "🔴"
    
    print(f"\n   {emoji} Risk Score: {risk_score}/100 ({level})")
    print(f"\n   Risk Factors:")
    for factor in risk_factors:
        print(f"      {factor}")
    
    return {
        "score": risk_score,
        "level": level,
        "factors": risk_factors
    }

async def main():
    print("=" * 60)
    print("🧪 Solana Token Analysis")
    print("=" * 60)
    print(f"⏰ Time: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print(f"🎯 Token: {TOKEN_ADDRESS}")
    
    # Get DexScreener data
    dex_data = await get_dexscreener_data(TOKEN_ADDRESS)
    
    # Get Solana RPC data
    await get_solana_token_info(TOKEN_ADDRESS)
    
    # Analyze risk
    risk = analyze_risk(dex_data)
    
    # Summary
    print("\n" + "=" * 60)
    print("📋 SUMMARY")
    print("=" * 60)
    
    if dex_data:
        name = dex_data.get("baseToken", {}).get("name", "Unknown")
        symbol = dex_data.get("baseToken", {}).get("symbol", "???")
        price = float(dex_data.get("priceUsd", 0))
        liquidity = dex_data.get("liquidity", {}).get("usd", 0)
        mcap = dex_data.get("marketCap", 0)
        
        print(f"   Token: {name} ({symbol})")
        print(f"   Price: ${price:.8f}")
        print(f"   Liquidity: ${liquidity:,.2f}")
        print(f"   Market Cap: ${mcap:,.2f}")
        print(f"   Risk Score: {risk['score']}/100 ({risk['level']})")
        
        # Trading recommendation
        print(f"\n   💡 Recommendation:")
        if risk['score'] <= 30:
            print(f"      ✅ Token appears relatively safe for trading")
        elif risk['score'] <= 50:
            print(f"      ⚠️ Proceed with caution, moderate risk detected")
        elif risk['score'] <= 70:
            print(f"      🟠 High risk - only trade with money you can afford to lose")
        else:
            print(f"      🔴 CRITICAL RISK - Avoid trading this token!")
    else:
        print("   ❌ Could not analyze token - no data available")

if __name__ == "__main__":
    asyncio.run(main())
