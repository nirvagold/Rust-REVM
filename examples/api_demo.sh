#!/bin/bash
# Ruster REVM Cloud API Demo
# Run: cargo run --bin ruster_api
# Then execute these curl commands

BASE_URL="http://localhost:3000/v1"

echo "=== Ruster REVM API Demo ==="
echo ""

# Health check
echo "1. Health Check:"
curl -s "$BASE_URL/health" | jq .
echo ""

# Honeypot check (USDT - should be safe)
echo "2. Honeypot Check (USDT):"
curl -s -X POST "$BASE_URL/honeypot/check" \
  -H "Content-Type: application/json" \
  -d '{"token_address": "0xdAC17F958D2ee523a2206206994597C13D831ec7", "test_amount_eth": "0.1"}' | jq .
echo ""

# Full token analysis
echo "3. Full PERS Analysis (DAI):"
curl -s -X POST "$BASE_URL/analyze/token" \
  -H "Content-Type: application/json" \
  -d '{"token_address": "0x6B175474E89094C44Da98b954EescdeCB5f8F4", "test_amount_eth": "0.1", "chain_id": 1}' | jq .
echo ""

# Batch analysis
echo "4. Batch Analysis (3 tokens):"
curl -s -X POST "$BASE_URL/analyze/batch" \
  -H "Content-Type: application/json" \
  -d '{
    "tokens": [
      "0xdAC17F958D2ee523a2206206994597C13D831ec7",
      "0x6B175474E89094C44Da98b954EescdeCB5f8F4",
      "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
    ],
    "test_amount_eth": "0.1",
    "concurrency": 3
  }' | jq .
echo ""

# Stats
echo "5. Protection Stats:"
curl -s "$BASE_URL/stats" | jq .
echo ""

echo "=== Demo Complete ==="
