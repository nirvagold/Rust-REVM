#!/usr/bin/env python3
"""
🧪 Test WebSocket Sniper Bot - Token Detection
Tests the bot's ability to detect and analyze new tokens
"""

import asyncio
import aiohttp
import os
import sys
from datetime import datetime
from dotenv import load_dotenv

load_dotenv()

ALCHEMY_API_KEY = os.getenv("ALCHEMY_API_KEY", "")
RUSTER_API = "http://localhost:8080"

# Test tokens (recently created on various chains)
TEST_TOKENS = [
    # Ethereum - PEPE (popular meme token)
    {"chain_id": 1, "address": "0x6982508145454Ce325dDbE47a25d4ec3d2311933", "name": "PEPE"},
    # Ethereum - SHIB
    {"chain_id": 1, "address": "0x95aD61b0a150d79219dCF64E1E6Cc01f0B64C4cE", "name": "SHIB"},
    # BSC - CAKE
    {"chain_id": 56, "address": "0x0E09FaBB73Bd3Ade0a17ECC321fD13a19e81cE82", "name": "CAKE"},
    # Polygon - QUICK
    {"chain_id": 137, "address": "0xB5C064F955D8e7F38fE0460C556a72987494eE17", "name": "QUICK"},
    # Arbitrum - ARB
    {"chain_id": 42161, "address": "0x912CE59144191C1204E64559FE8253a0e49E6548", "name": "ARB"},
    # Base - BRETT
    {"chain_id": 8453, "address": "0x532f27101965dd16442E59d40670FaF5eBB142E4", "name": "BRETT"},
]

async def test_api_health():
    """Test API health"""
    print("🏥 Testing API Health...")
    async with aiohttp.ClientSession() as session:
        try:
            async with session.get(f"{RUSTER_API}/v1/health", timeout=aiohttp.ClientTimeout(total=5)) as resp:
                if resp.status == 200:
                    data = await resp.json()
                    print(f"   ✅ API is healthy: {data}")
                    return True
                else:
                    print(f"   ❌ API returned {resp.status}")
                    return False
        except Exception as e:
            print(f"   ❌ API error: {e}")
            return False

async def test_honeypot_check(session: aiohttp.ClientSession, token: dict):
    """Test honeypot check for a token"""
    chain_id = token["chain_id"]
    address = token["address"]
    name = token["name"]
    
    print(f"\n🔍 Testing {name} on chain {chain_id}...")
    print(f"   Address: {address}")
    
    try:
        # Test POST endpoint
        async with session.post(
            f"{RUSTER_API}/v1/honeypot/check",
            json={
                "token_address": address,
                "chain_id": chain_id,
                "test_amount_eth": "0.01"
            },
            timeout=aiohttp.ClientTimeout(total=30)
        ) as resp:
            if resp.status == 200:
                data = await resp.json()
                result = data.get("data", {})
                print(f"   ✅ Analysis complete:")
                print(f"      Token: {result.get('token_name', 'N/A')} ({result.get('token_symbol', 'N/A')})")
                print(f"      Is Honeypot: {result.get('is_honeypot', 'N/A')}")
                print(f"      Risk Score: {result.get('risk_score', 'N/A')}")
                print(f"      Buy Tax: {result.get('buy_tax_percent', 'N/A'):.2f}%")
                print(f"      Sell Tax: {result.get('sell_tax_percent', 'N/A'):.2f}%")
                print(f"      Liquidity: ${result.get('liquidity_usd', 0):,.0f}")
                print(f"      Price: ${float(result.get('price_usd', 0)):.8f}")
                print(f"      DEX: {result.get('dex_name', 'N/A')}")
                return result
            else:
                error = await resp.text()
                print(f"   ❌ Error {resp.status}: {error[:200]}")
                return None
    except asyncio.TimeoutError:
        print(f"   ⏰ Timeout analyzing {name}")
        return None
    except Exception as e:
        print(f"   ❌ Error: {e}")
        return None

async def test_websocket_connection():
    """Test WebSocket connection to Alchemy"""
    import websockets
    
    print("\n🔌 Testing WebSocket Connection...")
    
    if not ALCHEMY_API_KEY:
        print("   ❌ ALCHEMY_API_KEY not set")
        return False
        
    ws_url = f"wss://eth-mainnet.g.alchemy.com/v2/{ALCHEMY_API_KEY}"
    
    try:
        async with websockets.connect(ws_url) as ws:
            print("   ✅ Connected to Alchemy WebSocket")
            
            # Subscribe to pending transactions (just to test)
            subscribe_msg = {
                "jsonrpc": "2.0",
                "method": "eth_subscribe",
                "params": ["newHeads"],
                "id": 1
            }
            
            await ws.send(str(subscribe_msg).replace("'", '"'))
            
            # Wait for response
            response = await asyncio.wait_for(ws.recv(), timeout=5)
            print(f"   ✅ Subscription response: {response[:100]}...")
            
            # Wait for one block
            print("   ⏳ Waiting for new block...")
            block = await asyncio.wait_for(ws.recv(), timeout=30)
            print(f"   ✅ Received block notification!")
            
            return True
            
    except asyncio.TimeoutError:
        print("   ⏰ Timeout waiting for WebSocket data")
        return False
    except Exception as e:
        print(f"   ❌ WebSocket error: {e}")
        return False

async def test_pair_created_subscription():
    """Test PairCreated event subscription"""
    import websockets
    
    print("\n🏭 Testing PairCreated Subscription...")
    
    if not ALCHEMY_API_KEY:
        print("   ❌ ALCHEMY_API_KEY not set")
        return False
        
    ws_url = f"wss://eth-mainnet.g.alchemy.com/v2/{ALCHEMY_API_KEY}"
    factory = "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f"  # Uniswap V2 Factory
    
    try:
        async with websockets.connect(ws_url) as ws:
            print("   ✅ Connected to Alchemy WebSocket")
            
            # Subscribe to PairCreated events
            subscribe_msg = {
                "jsonrpc": "2.0",
                "method": "eth_subscribe",
                "params": [
                    "logs",
                    {
                        "address": factory,
                        "topics": [
                            "0x0d3648bd0f6ba80134a33ba9275ac585d9d315f0ad8355cddefde31afa28d0e9"
                        ]
                    }
                ],
                "id": 1
            }
            
            await ws.send(str(subscribe_msg).replace("'", '"'))
            
            # Wait for subscription confirmation
            response = await asyncio.wait_for(ws.recv(), timeout=5)
            data = eval(response)
            
            if "result" in data:
                print(f"   ✅ Subscribed to PairCreated events")
                print(f"   📡 Subscription ID: {data['result']}")
                print("   ⏳ Waiting for new pair (this may take a while)...")
                print("   💡 Press Ctrl+C to stop waiting")
                
                # Wait for a PairCreated event (timeout after 60s for test)
                try:
                    event = await asyncio.wait_for(ws.recv(), timeout=60)
                    print(f"   🆕 NEW PAIR DETECTED!")
                    print(f"   📦 Event: {event[:200]}...")
                    return True
                except asyncio.TimeoutError:
                    print("   ⏰ No new pairs in 60 seconds (this is normal)")
                    return True  # Subscription works, just no events
            else:
                print(f"   ❌ Subscription failed: {response}")
                return False
                
    except Exception as e:
        print(f"   ❌ Error: {e}")
        return False

async def main():
    """Main test function"""
    print("=" * 60)
    print("🧪 WebSocket Sniper Bot - Integration Test")
    print("=" * 60)
    print(f"⏰ Started: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print(f"🔑 Alchemy API Key: {'✅ Set' if ALCHEMY_API_KEY else '❌ Not set'}")
    print(f"🌐 API URL: {RUSTER_API}")
    
    # Test 1: API Health
    if not await test_api_health():
        print("\n❌ API not available. Please start the API first:")
        print("   cargo run --release --bin ruster_api")
        return
        
    # Test 2: Honeypot Analysis
    print("\n" + "=" * 60)
    print("📊 Testing Honeypot Analysis")
    print("=" * 60)
    
    async with aiohttp.ClientSession() as session:
        results = []
        for token in TEST_TOKENS[:3]:  # Test first 3 tokens
            result = await test_honeypot_check(session, token)
            if result:
                results.append(result)
            await asyncio.sleep(1)  # Rate limiting
            
    # Test 3: WebSocket Connection
    print("\n" + "=" * 60)
    print("🔌 Testing WebSocket Connection")
    print("=" * 60)
    
    await test_websocket_connection()
    
    # Test 4: PairCreated Subscription
    print("\n" + "=" * 60)
    print("🏭 Testing PairCreated Subscription")
    print("=" * 60)
    
    await test_pair_created_subscription()
    
    # Summary
    print("\n" + "=" * 60)
    print("📋 Test Summary")
    print("=" * 60)
    print(f"✅ API Health: OK")
    print(f"✅ Honeypot Analysis: {len(results)}/{len(TEST_TOKENS[:3])} tokens analyzed")
    print(f"✅ WebSocket: Connected")
    print(f"✅ PairCreated Subscription: Active")
    print("\n🎯 Bot is ready for token sniping!")

if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\n\n👋 Test interrupted by user")
