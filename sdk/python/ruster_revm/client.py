"""
Ruster REVM API Client
"""

import httpx
from typing import Optional
from .models import RiskScore, HoneypotResult, TokenAnalysis, BatchAnalysisResult


class RusterClient:
    """
    Ruster REVM API Client - Pre-Execution Risk Scoring (PERS)
    
    Example:
        client = RusterClient(api_key="sk_live_xxx")
        
        # Quick honeypot check
        result = client.check_honeypot("0x...")
        
        # Full PERS analysis
        analysis = client.analyze_token("0x...", test_amount="0.5")
    """
    
    DEFAULT_BASE_URL = "https://api.ruster-revm.io/v1"
    
    def __init__(
        self,
        api_key: str,
        base_url: Optional[str] = None,
        timeout: float = 30.0,
    ):
        self.api_key = api_key
        self.base_url = base_url or self.DEFAULT_BASE_URL
        self.timeout = timeout
        self._client = httpx.Client(
            base_url=self.base_url,
            headers={"X-API-Key": api_key},
            timeout=timeout,
        )
    
    def check_honeypot(
        self,
        token_address: str,
        test_amount_eth: str = "0.1",
    ) -> HoneypotResult:
        """
        Quick honeypot detection via REVM simulation.
        
        Args:
            token_address: ERC20 token address (0x...)
            test_amount_eth: ETH amount for simulation (default: 0.1)
        
        Returns:
            HoneypotResult with is_honeypot, risk_score, taxes, etc.
        """
        response = self._client.post(
            "/honeypot/check",
            json={
                "token_address": token_address,
                "test_amount_eth": test_amount_eth,
            },
        )
        response.raise_for_status()
        return HoneypotResult.from_dict(response.json())
    
    def analyze_token(
        self,
        token_address: str,
        test_amount_eth: str = "0.1",
        chain_id: int = 1,
    ) -> TokenAnalysis:
        """
        Full PERS token risk analysis with granular scoring.
        
        Args:
            token_address: ERC20 token address
            test_amount_eth: ETH amount for simulation
            chain_id: 1=Mainnet, 42161=Arbitrum, 8453=Base
        
        Returns:
            TokenAnalysis with RiskScore (0-100), confidence, breakdown
        """
        response = self._client.post(
            "/analyze/token",
            json={
                "token_address": token_address,
                "test_amount_eth": test_amount_eth,
                "chain_id": chain_id,
            },
        )
        response.raise_for_status()
        return TokenAnalysis.from_dict(response.json())
    
    def get_risk_score(self, token_address: str) -> RiskScore:
        """Get PERS risk score for a token (0-100)."""
        analysis = self.analyze_token(token_address)
        return analysis.risk_score
    
    def batch_analyze(
        self,
        tokens: list[str],
        test_amount_eth: str = "0.1",
        chain_id: int = 1,
        concurrency: int = 10,
    ) -> "BatchAnalysisResult":
        """
        Analyze multiple tokens in a single request.
        
        Args:
            tokens: List of token addresses (max 100)
            test_amount_eth: ETH amount for simulation
            chain_id: Chain ID (1=Mainnet)
            concurrency: Max concurrent checks (1-50)
        
        Returns:
            BatchAnalysisResult with summary and individual results
        """
        response = self._client.post(
            "/analyze/batch",
            json={
                "tokens": tokens,
                "test_amount_eth": test_amount_eth,
                "chain_id": chain_id,
                "concurrency": concurrency,
            },
        )
        response.raise_for_status()
        return BatchAnalysisResult.from_dict(response.json())
    
    def is_safe(self, token_address: str, threshold: int = 40) -> bool:
        """Quick safety check. Returns True if risk_score <= threshold."""
        score = self.get_risk_score(token_address)
        return score.total <= threshold
    
    def close(self):
        """Close the HTTP client."""
        self._client.close()
    
    def __enter__(self):
        return self
    
    def __exit__(self, *args):
        self.close()


class AsyncRusterClient:
    """Async version of RusterClient."""
    
    def __init__(
        self,
        api_key: str,
        base_url: Optional[str] = None,
        timeout: float = 30.0,
    ):
        self.api_key = api_key
        self.base_url = base_url or RusterClient.DEFAULT_BASE_URL
        self._client = httpx.AsyncClient(
            base_url=self.base_url,
            headers={"X-API-Key": api_key},
            timeout=timeout,
        )
    
    async def check_honeypot(
        self,
        token_address: str,
        test_amount_eth: str = "0.1",
    ) -> HoneypotResult:
        response = await self._client.post(
            "/honeypot/check",
            json={
                "token_address": token_address,
                "test_amount_eth": test_amount_eth,
            },
        )
        response.raise_for_status()
        return HoneypotResult.from_dict(response.json())
    
    async def analyze_token(
        self,
        token_address: str,
        test_amount_eth: str = "0.1",
        chain_id: int = 1,
    ) -> TokenAnalysis:
        response = await self._client.post(
            "/analyze/token",
            json={
                "token_address": token_address,
                "test_amount_eth": test_amount_eth,
                "chain_id": chain_id,
            },
        )
        response.raise_for_status()
        return TokenAnalysis.from_dict(response.json())
    
    async def close(self):
        await self._client.aclose()
    
    async def __aenter__(self):
        return self
    
    async def __aexit__(self, *args):
        await self.close()
