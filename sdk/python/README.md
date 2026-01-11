# Ruster REVM Python SDK

Pre-Execution Risk Scoring (PERS) for Ethereum tokens in 3 lines of code.

## Installation

```bash
pip install ruster-revm
```

## Quick Start

```python
from ruster_revm import RusterClient

# Initialize client
client = RusterClient(api_key="sk_live_your_api_key")

# Check if token is honeypot
result = client.check_honeypot("0x...")

if result.is_honeypot:
    print(f"🚨 HONEYPOT DETECTED: {result.reason}")
else:
    print(f"✅ Safe - Tax: {result.total_loss_percent:.1f}%")
```

## Full PERS Analysis

```python
# Get detailed risk score (0-100)
analysis = client.analyze_token("0x...")

print(f"Risk Score: {analysis.risk_score.total}/100")
print(f"Confidence: {analysis.risk_score.confidence}%")
print(f"Recommendation: {analysis.risk_score.recommendation}")

# Check individual PERS components
print(f"Honeypot Risk: {analysis.risk_score.components.honeypot}")
print(f"Tax Risk: {analysis.risk_score.components.tax}")
print(f"MEV Exposure: {analysis.risk_score.components.mev_exposure}")
```

## Batch Analysis

Analyze up to 100 tokens in a single request:

```python
result = client.batch_analyze([
    "0xdAC17F958D2ee523a2206206994597C13D831ec7",  # USDT
    "0x6B175474E89094C44Da98b954EescdeCB5f8F4",    # DAI
    "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",  # USDC
], concurrency=10)

print(f"Processed: {result.total_processed}")
print(f"Safe: {result.total_safe}")
print(f"Honeypots: {result.total_honeypots}")
```

## Integration with Trading Bots

```python
from ruster_revm import RusterClient

client = RusterClient(api_key="sk_live_xxx")

def should_buy(token_address: str) -> bool:
    """Pre-trade PERS safety check."""
    if not client.is_safe(token_address, threshold=40):
        return False
    return True

# In your trading loop
if should_buy("0x..."):
    execute_buy()
```

## Support

- Documentation: https://docs.ruster-revm.io
- GitHub: https://github.com/nirvagold/ruster-revm
