"""
Ruster REVM Python SDK
Pre-Execution Risk Scoring (PERS) for Ethereum tokens

Usage:
    from ruster_revm import RusterClient
    
    client = RusterClient(api_key="your-api-key")
    result = client.check_honeypot("0x...")
    
    if result.is_honeypot:
        print("🚨 HONEYPOT DETECTED!")
    else:
        print(f"✅ Safe - Risk Score: {result.risk_score}/100")
"""

__version__ = "0.1.0"
__author__ = "Ruster REVM Team"

from .client import RusterClient
from .models import RiskScore, HoneypotResult, TokenAnalysis

__all__ = ["RusterClient", "RiskScore", "HoneypotResult", "TokenAnalysis"]
