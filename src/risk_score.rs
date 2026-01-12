//! Risk Scoring Module
//! Provides granular 0-100 risk scores instead of binary Safe/Honeypot
//!
//! This allows users to make informed decisions in "gray areas"

use serde::{Deserialize, Serialize};

/// Granular risk score (0-100)
/// - 0-20: Safe (green light)
/// - 21-40: Low Risk (proceed with caution)
/// - 41-60: Medium Risk (manual review recommended)
/// - 61-80: High Risk (likely dangerous)
/// - 81-100: Critical (almost certain loss)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskScore {
    /// Overall score (0-100)
    pub total: u8,
    /// Individual component scores
    pub components: RiskComponents,
    /// Confidence level (0-100) - how sure are we about this score?
    pub confidence: u8,
    /// Human-readable recommendation
    pub recommendation: String,
    /// Detailed breakdown for transparency
    pub breakdown: Vec<ScoreFactor>,
}

/// Individual risk components
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RiskComponents {
    /// Honeypot simulation score (0-100)
    pub honeypot: u8,
    /// Tax/fee score (0-100)
    pub tax: u8,
    /// Liquidity score (0-100) - low liquidity = higher risk
    pub liquidity: u8,
    /// Contract verification score (0-100)
    pub contract: u8,
    /// MEV exposure score (0-100)
    pub mev_exposure: u8,
}

/// Individual factor contributing to score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreFactor {
    pub name: String,
    pub score: u8,
    pub weight: f32,
    pub reason: String,
}

impl RiskScore {
    /// Create a new risk score from components
    pub fn calculate(components: RiskComponents, factors: Vec<ScoreFactor>) -> Self {
        // Weighted average calculation
        let weights = [
            (components.honeypot, 0.35),     // Honeypot is most critical
            (components.tax, 0.25),          // Tax is second
            (components.liquidity, 0.15),    // Liquidity matters
            (components.contract, 0.10),     // Contract verification
            (components.mev_exposure, 0.15), // MEV risk
        ];

        let total: f32 = weights
            .iter()
            .map(|(score, weight)| *score as f32 * weight)
            .sum();

        let total = (total.round() as u8).min(100);

        // Calculate confidence based on data availability
        let confidence = Self::calculate_confidence(&factors);

        let recommendation = Self::generate_recommendation(total, confidence);

        Self {
            total,
            components,
            confidence,
            recommendation,
            breakdown: factors,
        }
    }

    /// Calculate confidence level based on available data
    fn calculate_confidence(factors: &[ScoreFactor]) -> u8 {
        if factors.is_empty() {
            return 30; // Low confidence without data
        }

        // More factors = higher confidence
        let base_confidence = (factors.len() as u8 * 15).min(60);

        // Check for simulation-based factors (higher confidence)
        let has_simulation = factors
            .iter()
            .any(|f| f.name.contains("simulation") || f.name.contains("REVM"));

        let simulation_bonus = if has_simulation { 25 } else { 0 };

        (base_confidence + simulation_bonus).min(95)
    }

    /// Generate human-readable recommendation
    fn generate_recommendation(score: u8, confidence: u8) -> String {
        let risk_level = match score {
            0..=20 => "✅ LOW RISK",
            21..=40 => "🟡 MODERATE RISK",
            41..=60 => "🟠 ELEVATED RISK",
            61..=80 => "🔴 HIGH RISK",
            81..=100 => "💀 CRITICAL RISK",
            _ => "❓ UNKNOWN",
        };

        let confidence_note = match confidence {
            0..=40 => "(Low confidence - limited data)",
            41..=70 => "(Medium confidence)",
            71..=100 => "(High confidence - simulation verified)",
            _ => "",
        };

        let action = match score {
            0..=20 => "Proceed with standard caution.",
            21..=40 => "Review transaction details before proceeding.",
            41..=60 => "Manual review strongly recommended. Consider smaller test transaction.",
            61..=80 => "High probability of loss. Avoid unless you understand the risks.",
            81..=100 => "DO NOT PROCEED. Almost certain loss of funds.",
            _ => "Unable to assess.",
        };

        format!("{} {} - {}", risk_level, confidence_note, action)
    }

    /// Check if this is in the "gray area" requiring user decision
    pub fn is_gray_area(&self) -> bool {
        (30..=70).contains(&self.total) || self.confidence < 60
    }

    /// Get color code for UI
    pub fn color_code(&self) -> &'static str {
        match self.total {
            0..=20 => "#22c55e",   // Green
            21..=40 => "#eab308",  // Yellow
            41..=60 => "#f97316",  // Orange
            61..=80 => "#ef4444",  // Red
            81..=100 => "#7c2d12", // Dark red
            _ => "#6b7280",        // Gray
        }
    }
}

/// Builder for creating risk scores from analysis results
pub struct RiskScoreBuilder {
    factors: Vec<ScoreFactor>,
    components: RiskComponents,
}

impl RiskScoreBuilder {
    pub fn new() -> Self {
        Self {
            factors: Vec::new(),
            components: RiskComponents::default(),
        }
    }

    /// Add honeypot simulation result
    pub fn with_honeypot_result(
        mut self,
        is_honeypot: bool,
        sell_success: bool,
        loss_percent: f64,
    ) -> Self {
        let score = if is_honeypot {
            95
        } else if !sell_success {
            85
        } else if loss_percent > 50.0 {
            80
        } else if loss_percent > 30.0 {
            60
        } else if loss_percent > 15.0 {
            40
        } else if loss_percent > 5.0 {
            20
        } else {
            5
        };

        self.components.honeypot = score;
        self.factors.push(ScoreFactor {
            name: "REVM Honeypot simulation".to_string(),
            score,
            weight: 0.35,
            reason: format!(
                "Sell {}, Round-trip loss: {:.1}%",
                if sell_success { "succeeded" } else { "FAILED" },
                loss_percent
            ),
        });

        self
    }

    /// Add tax analysis result
    pub fn with_tax_analysis(mut self, buy_tax: f64, sell_tax: f64) -> Self {
        let total_tax = buy_tax + sell_tax;
        let score = if total_tax > 50.0 {
            90
        } else if total_tax > 30.0 {
            70
        } else if total_tax > 15.0 {
            50
        } else if total_tax > 5.0 {
            25
        } else {
            5
        };

        self.components.tax = score;
        self.factors.push(ScoreFactor {
            name: "Tax analysis".to_string(),
            score,
            weight: 0.25,
            reason: format!("Buy tax: {:.1}%, Sell tax: {:.1}%", buy_tax, sell_tax),
        });

        self
    }

    /// Add slippage analysis
    pub fn with_slippage(mut self, slippage_bps: u64) -> Self {
        let score = if slippage_bps > 2000 {
            90
        } else if slippage_bps > 1000 {
            70
        } else if slippage_bps > 500 {
            50
        } else if slippage_bps > 300 {
            30
        } else {
            10
        };

        self.components.mev_exposure = score;
        self.factors.push(ScoreFactor {
            name: "Slippage/MEV exposure".to_string(),
            score,
            weight: 0.15,
            reason: format!(
                "Slippage tolerance: {}bps ({:.1}%)",
                slippage_bps,
                slippage_bps as f64 / 100.0
            ),
        });

        self
    }

    /// Add contract verification status
    pub fn with_contract_verified(mut self, is_verified: bool) -> Self {
        let score = if is_verified { 10 } else { 60 };

        self.components.contract = score;
        self.factors.push(ScoreFactor {
            name: "Contract verification".to_string(),
            score,
            weight: 0.10,
            reason: if is_verified {
                "Contract source verified on Etherscan".to_string()
            } else {
                "Contract NOT verified - cannot audit code".to_string()
            },
        });

        self
    }

    /// Build final risk score
    pub fn build(self) -> RiskScore {
        RiskScore::calculate(self.components, self.factors)
    }
}

impl Default for RiskScoreBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_token_score() {
        let score = RiskScoreBuilder::new()
            .with_honeypot_result(false, true, 3.0)
            .with_tax_analysis(1.5, 1.5)
            .with_slippage(200)
            .with_contract_verified(true)
            .build();

        assert!(score.total <= 20);
        assert!(score.recommendation.contains("LOW RISK"));
    }

    #[test]
    fn test_honeypot_score() {
        let score = RiskScoreBuilder::new()
            .with_honeypot_result(true, false, 100.0)
            .with_tax_analysis(50.0, 50.0)
            .with_slippage(2500)
            .build();

        // Honeypot (95 * 0.35) + Tax (90 * 0.25) + MEV (90 * 0.15) = 33.25 + 22.5 + 13.5 = 69+
        assert!(score.total >= 60, "Score was {}", score.total);
        assert!(score.recommendation.contains("HIGH") || score.recommendation.contains("CRITICAL"));
    }

    #[test]
    fn test_gray_area_detection() {
        let score = RiskScoreBuilder::new()
            .with_honeypot_result(false, true, 25.0)
            .with_tax_analysis(10.0, 12.0)
            .build();

        assert!(score.is_gray_area());
    }
}
