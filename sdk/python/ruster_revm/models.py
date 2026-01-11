"""
Data models for Ruster REVM SDK
"""

from dataclasses import dataclass
from typing import List, Dict, Optional


@dataclass
class ScoreFactor:
    """Individual factor contributing to risk score."""
    name: str
    score: int
    weight: float
    reason: str
    
    @classmethod
    def from_dict(cls, data: dict) -> "ScoreFactor":
        return cls(
            name=data["name"],
            score=data["score"],
            weight=data["weight"],
            reason=data["reason"],
        )


@dataclass
class RiskComponents:
    """Individual risk component scores."""
    honeypot: int
    tax: int
    liquidity: int
    contract: int
    mev_exposure: int
    
    @classmethod
    def from_dict(cls, data: dict) -> "RiskComponents":
        return cls(
            honeypot=data.get("honeypot", 0),
            tax=data.get("tax", 0),
            liquidity=data.get("liquidity", 0),
            contract=data.get("contract", 0),
            mev_exposure=data.get("mev_exposure", 0),
        )


@dataclass
class RiskScore:
    """
    Granular risk score (0-100).
    
    Score ranges:
    - 0-20: Safe (green light)
    - 21-40: Low Risk (proceed with caution)
    - 41-60: Medium Risk (manual review recommended)
    - 61-80: High Risk (likely dangerous)
    - 81-100: Critical (almost certain loss)
    """
    total: int
    confidence: int
    recommendation: str
    is_gray_area: bool
    components: RiskComponents
    breakdown: List[ScoreFactor]
    
    @classmethod
    def from_dict(cls, data: dict) -> "RiskScore":
        return cls(
            total=data["total"],
            confidence=data["confidence"],
            recommendation=data["recommendation"],
            is_gray_area=data.get("is_gray_area", False),
            components=RiskComponents.from_dict(data.get("components", {})),
            breakdown=[ScoreFactor.from_dict(f) for f in data.get("breakdown", [])],
        )
    
    @property
    def is_safe(self) -> bool:
        """Returns True if risk score <= 20."""
        return self.total <= 20
    
    @property
    def is_dangerous(self) -> bool:
        """Returns True if risk score >= 61."""
        return self.total >= 61
    
    @property
    def level(self) -> str:
        """Human-readable risk level."""
        if self.total <= 20:
            return "SAFE"
        elif self.total <= 40:
            return "LOW"
        elif self.total <= 60:
            return "MEDIUM"
        elif self.total <= 80:
            return "HIGH"
        else:
            return "CRITICAL"


@dataclass
class HoneypotResult:
    """Result of honeypot detection."""
    is_honeypot: bool
    risk_score: int
    buy_success: bool
    sell_success: bool
    buy_tax_percent: float
    sell_tax_percent: float
    total_loss_percent: float
    reason: str
    latency_ms: float
    
    @classmethod
    def from_dict(cls, data: dict) -> "HoneypotResult":
        return cls(
            is_honeypot=data["is_honeypot"],
            risk_score=data.get("risk_score", 100 if data["is_honeypot"] else 0),
            buy_success=data["buy_success"],
            sell_success=data["sell_success"],
            buy_tax_percent=data["buy_tax_percent"],
            sell_tax_percent=data["sell_tax_percent"],
            total_loss_percent=data["total_loss_percent"],
            reason=data["reason"],
            latency_ms=data["latency_ms"],
        )
    
    @property
    def is_safe(self) -> bool:
        """Returns True if not a honeypot and total loss < 10%."""
        return not self.is_honeypot and self.total_loss_percent < 10.0
    
    def __str__(self) -> str:
        if self.is_honeypot:
            return f"🚨 HONEYPOT: {self.reason}"
        else:
            return f"✅ Safe (Loss: {self.total_loss_percent:.1f}%)"


@dataclass
class TokenAnalysis:
    """Full token analysis result."""
    success: bool
    risk_score: RiskScore
    latency_ms: float
    timestamp: int
    
    @classmethod
    def from_dict(cls, data: dict) -> "TokenAnalysis":
        return cls(
            success=data["success"],
            risk_score=RiskScore.from_dict(data["data"]),
            latency_ms=data["latency_ms"],
            timestamp=data["timestamp"],
        )
    
    @property
    def is_safe(self) -> bool:
        return self.risk_score.is_safe
    
    def __str__(self) -> str:
        return f"Risk: {self.risk_score.total}/100 ({self.risk_score.level})"


# ============================================
# Batch Analysis
# ============================================

@dataclass
class BatchTokenResult:
    """Result for a single token in batch analysis."""
    token_address: str
    status: str  # "success" | "error"
    risk_score: Optional[int]
    is_honeypot: Optional[bool]
    level: Optional[str]
    error: Optional[str]
    latency_ms: float
    
    @classmethod
    def from_dict(cls, data: dict) -> "BatchTokenResult":
        return cls(
            token_address=data["token_address"],
            status=data["status"],
            risk_score=data.get("risk_score"),
            is_honeypot=data.get("is_honeypot"),
            level=data.get("level"),
            error=data.get("error"),
            latency_ms=data["latency_ms"],
        )
    
    @property
    def is_safe(self) -> bool:
        """Returns True if risk_score <= 40 and not honeypot."""
        if self.is_honeypot:
            return False
        return self.risk_score is not None and self.risk_score <= 40


@dataclass
class BatchAnalysisResult:
    """Result of batch token analysis."""
    total_requested: int
    total_processed: int
    total_safe: int
    total_risky: int
    total_honeypots: int
    results: List[BatchTokenResult]
    processing_time_ms: float
    
    @classmethod
    def from_dict(cls, data: dict) -> "BatchAnalysisResult":
        inner = data.get("data", data)
        return cls(
            total_requested=inner["total_requested"],
            total_processed=inner["total_processed"],
            total_safe=inner["total_safe"],
            total_risky=inner["total_risky"],
            total_honeypots=inner["total_honeypots"],
            results=[BatchTokenResult.from_dict(r) for r in inner["results"]],
            processing_time_ms=inner["processing_time_ms"],
        )
    
    @property
    def safe_tokens(self) -> List[BatchTokenResult]:
        """Returns list of safe tokens."""
        return [r for r in self.results if r.is_safe]
    
    @property
    def honeypots(self) -> List[BatchTokenResult]:
        """Returns list of detected honeypots."""
        return [r for r in self.results if r.is_honeypot]
    
    def __str__(self) -> str:
        return (
            f"Batch Analysis: {self.total_processed}/{self.total_requested} processed | "
            f"Safe: {self.total_safe} | Risky: {self.total_risky} | "
            f"Honeypots: {self.total_honeypots} | Time: {self.processing_time_ms:.1f}ms"
        )
