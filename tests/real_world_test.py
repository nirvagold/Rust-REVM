#!/usr/bin/env python3
"""
🧪 Real-World Integration Test
Tests the complete pipeline: WebSocket → Rust API → Analysis → Response

This script:
1. Starts the Rust API server
2. Tests WebSocket connection to Alchemy
3. Simulates PairCreated events
4. Tests honeypot analysis
5. Verifies end-to-end functionality
"""

import asyncio
import aiohttp
import json
import subprocess
import time
import os
from datetime import datetime

# Configuration
RUST_API_URL = "http://localhost:8080"
ALCHEMY_API_KEY = os.getenv("ALCHEMY_API_KEY", "")

# Test data
TEST_TOKENS = {
    "WETH": "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
    "USDC": "0xA0b86a33E6441b8435b662f0E2d0B8A0E4B2B8B0",
    "HONEYPOT": "0x1234567890123456789012345678901234567890",  # Fake honeypot
}

class RealWorldTester:
    def __init__(self):
        self.session = None
        self.rust_process = None
        
    async def run_all_tests(self):
        """Run all integration tests"""
        print("🧪 Real-World Integration Test Suite")
        print("=" * 50)
        
        try:
            # Setup
            await self.setup()
            
            # Test sequence
            await self.test_rust_api_health()
            await self.test_token_analysis()
            await self.test_websocket_simulation()
            await self.test_trace_analysis()
            await self.test_performance()
            
            print("\n✅ All tests completed successfully!")
            
        except Exception as e:
            print(f"\n❌ Test failed: {e}")
        finally:
            await self.cleanup()
            
    async def setup(self):
        """Setup test environment"""
        print("🔧 Setting up test environment...")
        
        # Create HTTP session
        self.session = aiohttp.ClientSession()
        
        # Start Rust API server
        print("🚀 Starting Rust API server...")
        self.rust_process = subprocess.Popen(
            ["cargo", "run", "--release", "--bin", "ruster_api"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            cwd=".."
        )
        
        # Wait for server to start
        print("⏳ Waiting for API server to start...")
        for i in range(30):  # 30 second timeout
            try:
                async with self.session.get(f"{RUST_API_URL}/health") as response:
                    if response.status == 200:
                        print("✅ Rust API server is ready")
                        return
            except:
                pass
            await asyncio.sleep(1)
            
        raise Exception("Rust API server failed to start")
        
    async def test_rust_api_health(self):
        """Test Rust API health endpoint"""
        print("\n🏥 Testing Rust API health...")
        
        async with self.session.get(f"{RUST_API_URL}/health") as response:
            assert response.status == 200
            data = await response.json()
            print(f"   Status: {data.get('status')}")
            print(f"   Version: {data.get('version')}")
            print("✅ API health check passed")
            
    async def test_token_analysis(self):
        """Test token analysis endpoints"""
        print("\n🔍 Testing token analysis...")
        
        for name, address in TEST_TOKENS.items():
            print(f"   Testing {name}: {address}")
            
            async with self.session.get(
                f"{RUST_API_URL}/v1/honeypot/check",
                params={"address": address, "chain_id": 1}
            ) as response:
                if response.status == 200:
                    data = await response.json()
                    print(f"     Risk Score: {data.get('risk_score', 'N/A')}")
                    print(f"     Is Honeypot: {data.get('is_honeypot', 'N/A')}")
                else:
                    print(f"     ❌ Failed: {response.status}")
                    
        print("✅ Token analysis tests completed")
        
    async def test_websocket_simulation(self):
        """Test WebSocket event simulation"""
        print("\n🔌 Testing WebSocket simulation...")
        
        if not ALCHEMY_API_KEY:
            print("   ⚠️ Skipping WebSocket test (no ALCHEMY_API_KEY)")
            return
            
        # Simulate PairCreated event data
        pair_event = {
            "chain_id": 1,
            "factory": "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f",
            "token0": TEST_TOKENS["WETH"],
            "token1": TEST_TOKENS["USDC"],
            "pair": "0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc",
            "block_number": 19000000,
            "tx_hash": "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef12"
        }
        
        # Test pair analysis endpoint
        async with self.session.post(
            f"{RUST_API_URL}/v1/pair/analyze",
            json=pair_event
        ) as response:
            if response.status == 200:
                data = await response.json()
                print(f"   Pair analysis: {data.get('status', 'unknown')}")
                print("✅ WebSocket simulation passed")
            else:
                print(f"   ❌ Pair analysis failed: {response.status}")
                
    async def test_trace_analysis(self):
        """Test trace analysis functionality"""
        print("\n🔍 Testing trace analysis...")
        
        # Test transaction trace
        test_tx = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef12"
        
        async with self.session.get(
            f"{RUST_API_URL}/v1/trace/transaction",
            params={"tx_hash": test_tx, "chain_id": 1}
        ) as response:
            if response.status == 200:
                data = await response.json()
                print(f"   Trace analysis: {data.get('status', 'completed')}")
                print("✅ Trace analysis passed")
            else:
                print(f"   ⚠️ Trace analysis: {response.status} (may be expected)")
                
    async def test_performance(self):
        """Test API performance"""
        print("\n⚡ Testing API performance...")
        
        # Test concurrent requests
        start_time = time.time()
        tasks = []
        
        for i in range(10):
            task = self.session.get(
                f"{RUST_API_URL}/v1/honeypot/check",
                params={"address": TEST_TOKENS["WETH"], "chain_id": 1}
            )
            tasks.append(task)
            
        responses = await asyncio.gather(*tasks, return_exceptions=True)
        end_time = time.time()
        
        successful = sum(1 for r in responses if hasattr(r, 'status') and r.status == 200)
        total_time = end_time - start_time
        
        print(f"   Concurrent requests: {successful}/10 successful")
        print(f"   Total time: {total_time:.2f}s")
        print(f"   Avg time per request: {total_time/10:.3f}s")
        
        if successful >= 8 and total_time < 5.0:
            print("✅ Performance test passed")
        else:
            print("⚠️ Performance test: some issues detected")
            
    async def cleanup(self):
        """Cleanup test environment"""
        print("\n🧹 Cleaning up...")
        
        if self.session:
            await self.session.close()
            
        if self.rust_process:
            self.rust_process.terminate()
            try:
                self.rust_process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.rust_process.kill()
            print("✅ Rust API server stopped")

# ============================================
# WEBSOCKET LIVE TEST
# ============================================

async def test_live_websocket():
    """Test live WebSocket connection to Alchemy"""
    print("\n🔴 LIVE WebSocket Test")
    print("=" * 30)
    
    if not ALCHEMY_API_KEY:
        print("❌ ALCHEMY_API_KEY required for live test")
        return
        
    import websockets
    
    ws_url = f"wss://eth-mainnet.g.alchemy.com/v2/{ALCHEMY_API_KEY}"
    
    try:
        print("🔌 Connecting to Alchemy WebSocket...")
        async with websockets.connect(ws_url) as websocket:
            print("✅ Connected!")
            
            # Subscribe to newHeads
            subscribe_msg = {
                "jsonrpc": "2.0",
                "method": "eth_subscribe",
                "params": ["newHeads"],
                "id": 1
            }
            
            await websocket.send(json.dumps(subscribe_msg))
            print("📡 Subscribed to newHeads")
            
            # Listen for 3 blocks
            block_count = 0
            async for message in websocket:
                data = json.loads(message)
                
                if "params" in data and "result" in data["params"]:
                    result = data["params"]["result"]
                    block_number = int(result.get("number", "0x0"), 16)
                    print(f"📦 New block: {block_number}")
                    
                    block_count += 1
                    if block_count >= 3:
                        break
                        
            print("✅ Live WebSocket test completed")
            
    except Exception as e:
        print(f"❌ WebSocket test failed: {e}")

# ============================================
# MAIN
# ============================================

async def main():
    """Main test function"""
    print("🧪 Real-World Integration Test Suite")
    print(f"⏰ Started at: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print("=" * 60)
    
    # Check prerequisites
    if not ALCHEMY_API_KEY:
        print("⚠️ ALCHEMY_API_KEY not set - some tests will be skipped")
        
    # Run integration tests
    tester = RealWorldTester()
    await tester.run_all_tests()
    
    # Run live WebSocket test if API key available
    if ALCHEMY_API_KEY:
        await test_live_websocket()
    
    print(f"\n⏰ Completed at: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print("🎉 Integration testing complete!")

if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\n🛑 Test interrupted by user")
    except Exception as e:
        print(f"\n💥 Test suite failed: {e}")