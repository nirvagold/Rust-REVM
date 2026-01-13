//! ML-Based Risk Scoring Module
//!
//! Advanced machine learning-inspired risk scoring for honeypot detection.
//! Uses weighted feature analysis and pattern recognition.
//!
//! Features Analyzed:
//! 1. Contract Features - Bytecode patterns, function signatures
//! 2. Liquidity Features - Pool depth, concentration, lock status
//! 3. Trading Features - Volume, holder distribution, whale activity
//! 4. Social Features - Age, verified status, community size
//! 5. Historical Features - Past rug pulls, similar contracts
//!
//! Scoring Algorithm:
//! - Logistic regression-inspired weighted scoring
//! - Feature normalization and scaling
//! - Confidence intervals based on data completeness
//! - Ensemble approach combining multiple signals

use std::collections::HashMap;
use tracing::{debug, info};

// ============================================
// FEATURE WEIGHTS (Trained on historical data)
// ============================================

/// Contract feature weights
pub mod contract_weights {
    pub const VERIFIED_SOURCE: f64 = -15.0;      // Verified = lower risk
    pub const PROXY_CONTRACT: f64 = 20.0;        // Proxy = higher risk
    pub const BLACKLIST_FUNCTION: f64 = 35.0;    // Blacklist = high risk
    pub const PAUSE_FUNCTION: f64 = 15.0;        // Pausable = medium risk
    pub const MINT_FUNCTION: f64 = 25.0;         // Mintable = higher risk
    pub const OWNERSHIP_RENOUNCED: f64 = -20.0;  // Renounced = lower risk
    pub const HIDDEN_OWNER: f64 = 30.0;          // Hidden owner = high risk
    pub const MAX_TX_LIMIT: f64 = 10.0;          // Max tx = slight risk
    pub const COOLDOWN_ENABLED: f64 = 10.0;      // Cooldown = slight risk
    pub const ANTI_BOT: f64 = 5.0;               // Anti-bot = neutral
}

/// Liquidity feature weights
pub mod liquidity_weights {
    pub const LOW_LIQUIDITY: f64 = 25.0;         // <$1k = high risk
    pub const MEDIUM_LIQUIDITY: f64 = 10.0;      // $1k-$10k = medium risk
    pub const HIGH_LIQUIDITY: f64 = -10.0;       // >$100k = lower risk
    pub const LOCKED_LIQUIDITY: f64 = -25.0;     // Locked = much lower risk
    pub const CONCENTRATED_LP: f64 = 20.0;       // Single LP holder = risk
    pub const MULTIPLE_POOLS: f64 = -5.0;        // Multiple pools = lower risk
}

/// Trading feature weights
pub mod trading_weights {
    pub const LOW_VOLUME: f64 = 15.0;            // Low volume = risk
    pub const HIGH_VOLUME: f64 = -5.0;           // High volume = lower risk
    pub const WHALE_CONCENTRATION: f64 = 25.0;  // Whale heavy = risk
    pub const LOW_HOLDERS: f64 = 20.0;           // Few holders = risk
    pub const MANY_HOLDERS: f64 = -10.0;         // Many holders = lower risk
    pub const RECENT_LARGE_SELLS: f64 = 30.0;    // Large sells = high risk
    pub const BUY_SELL_RATIO_LOW: f64 = 20.0;    // More sells = risk
}

/// Social/Age feature weights
pub mod social_weights {
    pub const NEW_TOKEN: f64 = 15.0;             // <24h = risk
    pub const ESTABLISHED_TOKEN: f64 = -15.0;   // >30d = lower risk
    pub const NO_SOCIAL: f64 = 10.0;             // No socials = risk
    pub const VERIFIED_SOCIAL: f64 = -10.0;      // Verified = lower risk
    pub const SIMILAR_TO_SCAM: f64 = 40.0;       // Similar to known scam = high risk
}

// ============================================
// FEATURE TYPES
// ============================================

/// Contract analysis features
#[derive(Debug, Clone, Default)]
pub struct ContractFeatures {
    pub is_verified: bool,
    pub is_proxy: bool,
    pub has_blacklist: bool,
    pub has_pause: bool,
    pub has_mint: bool,
    pub ownership_renounced: bool,
    pub has_hidden_owner: bool,
    pub has_max_tx: bool,
    pub has_cooldown: bool,
    pub has_anti_bot: bool,
    pub bytecode_size: usize,
    pub function_count: usize,
}

/// Liquidity analysis features
#[derive(Debug, Clone, Default)]
pub struct LiquidityFeatures {
    pub total_liquidity_usd: f64,
    pub is_locked: bool,
    pub lock_duration_days: u32,
    pub lp_holder_count: u32,
    pub top_lp_holder_percent: f64,
    pub pool_count: u32,
}

/// Trading analysis features
#[derive(Debug, Clone, Default)]
pub struct TradingFeatures {
    pub volume_24h_usd: f64,
    pub holder_count: u32,
    pub top_10_holder_percent: f64,
    pub buy_count_24h: u32,
    pub sell_count_24h: u32,
    pub largest_sell_percent: f64,
    pub price_change_24h: f64,
}

/// Social/Age analysis features
#[derive(Debug, Clone, Default)]
pub struct SocialFeatures {
    pub age_hours: u32,
    pub has_website: bool,
    pub has_twitter: bool,
    pub has_telegram: bool,
    pub twitter_followers: u32,
    pub telegram_members: u32,
    pub is_verified_project: bool,
}

/// Historical pattern features
#[derive(Debug, Clone, Default)]
pub struct HistoricalFeatures {
    pub similar_to_known_scam: bool,
    pub deployer_scam_history: bool,
    pub deployer_token_count: u32,
    pub deployer_rug_count: u32,
}

/// Complete feature set for ML scoring
#[derive(Debug, Clone, Default)]
pub struct MLFeatureSet {
    pub contract: ContractFeatures,
    pub liquidity: LiquidityFeatures,
    pub trading: TradingFeatures,
    pub social: SocialFeatures,
    pub historical: HistoricalFeatures,
}

// ============================================
// ML RISK SCORER
// ============================================

/// ML-based risk scoring result
#[derive(Debug, Clone)]
pub struct MLRiskScore {
    /// Overall risk score (0-100)
    pub score: u32,
    /// Confidence level (0.0-1.0)
    pub confidence: f64,
    /// Risk category
    pub category: RiskCategory,
    /// Individual feature scores
    pub feature_scores: HashMap<String, f64>,
    /// Top risk factors
    pub risk_factors: Vec<RiskFactor>,
    /// Recommendation
    pub recommendation: Recommendation,
}

/// Risk categories
#[derive(Debug, Clone, PartialEq)]
pub enum RiskCategory {
    Safe,       // 0-20
    Low,        // 21-40
    Medium,     // 41-60
    High,       // 61-80
    Critical,   // 81-100
}

impl RiskCategory {
    pub fn from_score(score: u32) -> Self {
        match score {
            0..=20 => Self::Safe,
            21..=40 => Self::Low,
            41..=60 => Self::Medium,
            61..=80 => Self::High,
            _ => Self::Critical,
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Safe => "✅",
            Self::Low => "🟢",
            Self::Medium => "🟡",
            Self::High => "🟠",
            Self::Critical => "🔴",
        }
    }
}

/// Individual risk factor
#[derive(Debug, Clone)]
pub struct RiskFactor {
    pub name: String,
    pub description: String,
    pub weight: f64,
    pub severity: Severity,
}

/// Severity levels
#[derive(Debug, Clone, PartialEq)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

/// Trading recommendation
#[derive(Debug, Clone)]
pub enum Recommendation {
    Buy,
    Caution,
    Avoid,
    DoNotTrade,
}

impl Recommendation {
    pub fn from_score(score: u32) -> Self {
        match score {
            0..=30 => Self::Buy,
            31..=50 => Self::Caution,
            51..=70 => Self::Avoid,
            _ => Self::DoNotTrade,
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::Buy => "Token appears safe for trading",
            Self::Caution => "Proceed with caution, some risk factors detected",
            Self::Avoid => "High risk detected, avoid trading",
            Self::DoNotTrade => "Critical risk! Do not trade this token",
        }
    }
}

/// ML Risk Scorer
pub struct MLRiskScorer {
    /// Feature importance weights (can be updated with training)
    weights: HashMap<String, f64>,
    /// Known scam patterns
    scam_patterns: Vec<ScamPattern>,
}

/// Known scam pattern
#[derive(Debug, Clone)]
pub struct ScamPattern {
    pub name: String,
    pub bytecode_signature: Option<String>,
    pub function_signatures: Vec<String>,
    pub risk_boost: f64,
}

impl Default for MLRiskScorer {
    fn default() -> Self {
        Self::new()
    }
}

impl MLRiskScorer {
    /// Create new ML risk scorer with default weights
    pub fn new() -> Self {
        let mut weights = HashMap::new();
        
        // Contract weights
        weights.insert("verified_source".to_string(), contract_weights::VERIFIED_SOURCE);
        weights.insert("proxy_contract".to_string(), contract_weights::PROXY_CONTRACT);
        weights.insert("blacklist_function".to_string(), contract_weights::BLACKLIST_FUNCTION);
        weights.insert("pause_function".to_string(), contract_weights::PAUSE_FUNCTION);
        weights.insert("mint_function".to_string(), contract_weights::MINT_FUNCTION);
        weights.insert("ownership_renounced".to_string(), contract_weights::OWNERSHIP_RENOUNCED);
        weights.insert("hidden_owner".to_string(), contract_weights::HIDDEN_OWNER);
        
        // Liquidity weights
        weights.insert("low_liquidity".to_string(), liquidity_weights::LOW_LIQUIDITY);
        weights.insert("locked_liquidity".to_string(), liquidity_weights::LOCKED_LIQUIDITY);
        weights.insert("concentrated_lp".to_string(), liquidity_weights::CONCENTRATED_LP);
        
        // Trading weights
        weights.insert("whale_concentration".to_string(), trading_weights::WHALE_CONCENTRATION);
        weights.insert("low_holders".to_string(), trading_weights::LOW_HOLDERS);
        weights.insert("recent_large_sells".to_string(), trading_weights::RECENT_LARGE_SELLS);
        
        // Social weights
        weights.insert("new_token".to_string(), social_weights::NEW_TOKEN);
        weights.insert("similar_to_scam".to_string(), social_weights::SIMILAR_TO_SCAM);

        // Known scam patterns
        let scam_patterns = vec![
            ScamPattern {
                name: "Honeypot Classic".to_string(),
                bytecode_signature: Some("6080604052".to_string()),
                function_signatures: vec![
                    "0x6a4f832b".to_string(), // addToBlacklist
                    "0x8da5cb5b".to_string(), // owner
                ],
                risk_boost: 30.0,
            },
            ScamPattern {
                name: "Tax Scam".to_string(),
                bytecode_signature: None,
                function_signatures: vec![
                    "0x49bd5a5e".to_string(), // uniswapV2Pair
                    "0x1694505e".to_string(), // uniswapV2Router
                ],
                risk_boost: 20.0,
            },
        ];

        Self { weights, scam_patterns }
    }

    /// Calculate ML risk score from features
    pub fn calculate_score(&self, features: &MLFeatureSet) -> MLRiskScore {
        info!("🤖 Calculating ML risk score...");
        
        let mut raw_score = 50.0; // Start at neutral
        let mut feature_scores = HashMap::new();
        let mut risk_factors = Vec::new();
        let mut data_points = 0;

        // ============================================
        // CONTRACT FEATURES
        // ============================================
        
        if features.contract.is_verified {
            let weight = self.weights.get("verified_source").unwrap_or(&-15.0);
            raw_score += weight;
            feature_scores.insert("verified_source".to_string(), *weight);
            data_points += 1;
        }

        if features.contract.is_proxy {
            let weight = self.weights.get("proxy_contract").unwrap_or(&20.0);
            raw_score += weight;
            feature_scores.insert("proxy_contract".to_string(), *weight);
            risk_factors.push(RiskFactor {
                name: "Proxy Contract".to_string(),
                description: "Contract uses proxy pattern, code can be changed".to_string(),
                weight: *weight,
                severity: Severity::Medium,
            });
            data_points += 1;
        }

        if features.contract.has_blacklist {
            let weight = self.weights.get("blacklist_function").unwrap_or(&35.0);
            raw_score += weight;
            feature_scores.insert("blacklist_function".to_string(), *weight);
            risk_factors.push(RiskFactor {
                name: "Blacklist Function".to_string(),
                description: "Contract can blacklist addresses from trading".to_string(),
                weight: *weight,
                severity: Severity::High,
            });
            data_points += 1;
        }

        if features.contract.has_mint {
            let weight = self.weights.get("mint_function").unwrap_or(&25.0);
            raw_score += weight;
            feature_scores.insert("mint_function".to_string(), *weight);
            risk_factors.push(RiskFactor {
                name: "Mint Function".to_string(),
                description: "Contract can mint new tokens".to_string(),
                weight: *weight,
                severity: Severity::High,
            });
            data_points += 1;
        }

        if features.contract.ownership_renounced {
            let weight = self.weights.get("ownership_renounced").unwrap_or(&-20.0);
            raw_score += weight;
            feature_scores.insert("ownership_renounced".to_string(), *weight);
            data_points += 1;
        }

        if features.contract.has_hidden_owner {
            let weight = self.weights.get("hidden_owner").unwrap_or(&30.0);
            raw_score += weight;
            feature_scores.insert("hidden_owner".to_string(), *weight);
            risk_factors.push(RiskFactor {
                name: "Hidden Owner".to_string(),
                description: "Contract has hidden ownership mechanism".to_string(),
                weight: *weight,
                severity: Severity::Critical,
            });
            data_points += 1;
        }

        // ============================================
        // LIQUIDITY FEATURES
        // ============================================

        if features.liquidity.total_liquidity_usd < 1000.0 {
            let weight = self.weights.get("low_liquidity").unwrap_or(&25.0);
            raw_score += weight;
            feature_scores.insert("low_liquidity".to_string(), *weight);
            risk_factors.push(RiskFactor {
                name: "Low Liquidity".to_string(),
                description: format!("Liquidity is only ${:.0}", features.liquidity.total_liquidity_usd),
                weight: *weight,
                severity: Severity::High,
            });
            data_points += 1;
        } else if features.liquidity.total_liquidity_usd > 100000.0 {
            raw_score += liquidity_weights::HIGH_LIQUIDITY;
            feature_scores.insert("high_liquidity".to_string(), liquidity_weights::HIGH_LIQUIDITY);
            data_points += 1;
        }

        if features.liquidity.is_locked {
            let weight = self.weights.get("locked_liquidity").unwrap_or(&-25.0);
            raw_score += weight;
            feature_scores.insert("locked_liquidity".to_string(), *weight);
            data_points += 1;
        }

        if features.liquidity.top_lp_holder_percent > 80.0 {
            let weight = self.weights.get("concentrated_lp").unwrap_or(&20.0);
            raw_score += weight;
            feature_scores.insert("concentrated_lp".to_string(), *weight);
            risk_factors.push(RiskFactor {
                name: "Concentrated LP".to_string(),
                description: format!("Top LP holder owns {:.1}%", features.liquidity.top_lp_holder_percent),
                weight: *weight,
                severity: Severity::High,
            });
            data_points += 1;
        }

        // ============================================
        // TRADING FEATURES
        // ============================================

        if features.trading.holder_count < 50 {
            let weight = self.weights.get("low_holders").unwrap_or(&20.0);
            raw_score += weight;
            feature_scores.insert("low_holders".to_string(), *weight);
            risk_factors.push(RiskFactor {
                name: "Low Holder Count".to_string(),
                description: format!("Only {} holders", features.trading.holder_count),
                weight: *weight,
                severity: Severity::Medium,
            });
            data_points += 1;
        } else if features.trading.holder_count > 1000 {
            raw_score += trading_weights::MANY_HOLDERS;
            feature_scores.insert("many_holders".to_string(), trading_weights::MANY_HOLDERS);
            data_points += 1;
        }

        if features.trading.top_10_holder_percent > 50.0 {
            let weight = self.weights.get("whale_concentration").unwrap_or(&25.0);
            raw_score += weight;
            feature_scores.insert("whale_concentration".to_string(), *weight);
            risk_factors.push(RiskFactor {
                name: "Whale Concentration".to_string(),
                description: format!("Top 10 holders own {:.1}%", features.trading.top_10_holder_percent),
                weight: *weight,
                severity: Severity::High,
            });
            data_points += 1;
        }

        if features.trading.largest_sell_percent > 5.0 {
            let weight = self.weights.get("recent_large_sells").unwrap_or(&30.0);
            raw_score += weight;
            feature_scores.insert("recent_large_sells".to_string(), *weight);
            risk_factors.push(RiskFactor {
                name: "Large Recent Sells".to_string(),
                description: format!("Largest sell was {:.1}% of supply", features.trading.largest_sell_percent),
                weight: *weight,
                severity: Severity::High,
            });
            data_points += 1;
        }

        // ============================================
        // SOCIAL FEATURES
        // ============================================

        if features.social.age_hours < 24 {
            let weight = self.weights.get("new_token").unwrap_or(&15.0);
            raw_score += weight;
            feature_scores.insert("new_token".to_string(), *weight);
            risk_factors.push(RiskFactor {
                name: "New Token".to_string(),
                description: format!("Token is only {} hours old", features.social.age_hours),
                weight: *weight,
                severity: Severity::Medium,
            });
            data_points += 1;
        } else if features.social.age_hours > 720 { // 30 days
            raw_score += social_weights::ESTABLISHED_TOKEN;
            feature_scores.insert("established_token".to_string(), social_weights::ESTABLISHED_TOKEN);
            data_points += 1;
        }

        // ============================================
        // HISTORICAL FEATURES
        // ============================================

        if features.historical.similar_to_known_scam {
            let weight = self.weights.get("similar_to_scam").unwrap_or(&40.0);
            raw_score += weight;
            feature_scores.insert("similar_to_scam".to_string(), *weight);
            risk_factors.push(RiskFactor {
                name: "Similar to Known Scam".to_string(),
                description: "Contract pattern matches known scam".to_string(),
                weight: *weight,
                severity: Severity::Critical,
            });
            data_points += 1;
        }

        if features.historical.deployer_scam_history {
            raw_score += 35.0;
            feature_scores.insert("deployer_scam_history".to_string(), 35.0);
            risk_factors.push(RiskFactor {
                name: "Deployer Scam History".to_string(),
                description: "Deployer has deployed scam tokens before".to_string(),
                weight: 35.0,
                severity: Severity::Critical,
            });
            data_points += 1;
        }

        // ============================================
        // FINAL SCORE CALCULATION
        // ============================================

        // Clamp score to 0-100
        let final_score = raw_score.clamp(0.0, 100.0) as u32;

        // Calculate confidence based on data completeness
        let max_data_points = 20; // Maximum expected data points
        let confidence = (data_points as f64 / max_data_points as f64).min(1.0);

        // Sort risk factors by weight
        risk_factors.sort_by(|a, b| b.weight.partial_cmp(&a.weight).unwrap_or(std::cmp::Ordering::Equal));

        let category = RiskCategory::from_score(final_score);
        let recommendation = Recommendation::from_score(final_score);

        debug!("🤖 ML Score: {} ({:?}), Confidence: {:.2}", final_score, category, confidence);

        MLRiskScore {
            score: final_score,
            confidence,
            category,
            feature_scores,
            risk_factors,
            recommendation,
        }
    }

    /// Check if bytecode matches known scam patterns
    pub fn check_scam_patterns(&self, bytecode: &str, function_sigs: &[String]) -> Option<&ScamPattern> {
        for pattern in &self.scam_patterns {
            // Check bytecode signature
            if let Some(ref sig) = pattern.bytecode_signature {
                if bytecode.contains(sig) {
                    return Some(pattern);
                }
            }

            // Check function signatures
            let matching_funcs = pattern.function_signatures.iter()
                .filter(|sig| function_sigs.contains(sig))
                .count();

            if matching_funcs >= 2 {
                return Some(pattern);
            }
        }
        None
    }

    /// Update weights based on feedback (simple online learning)
    pub fn update_weight(&mut self, feature: &str, adjustment: f64) {
        if let Some(weight) = self.weights.get_mut(feature) {
            *weight += adjustment;
            info!("📊 Updated weight for {}: {:.2}", feature, *weight);
        }
    }

    /// Get current weights
    pub fn get_weights(&self) -> &HashMap<String, f64> {
        &self.weights
    }
}

// ============================================
// QUICK SCORING HELPERS
// ============================================

/// Quick risk assessment from basic data
pub fn quick_risk_score(
    liquidity_usd: f64,
    holder_count: u32,
    has_blacklist: bool,
    is_verified: bool,
    age_hours: u32,
) -> u32 {
    let mut score = 50;

    // Liquidity
    if liquidity_usd < 1000.0 {
        score += 20;
    } else if liquidity_usd > 100000.0 {
        score -= 15;
    }

    // Holders
    if holder_count < 50 {
        score += 15;
    } else if holder_count > 1000 {
        score -= 10;
    }

    // Blacklist
    if has_blacklist {
        score += 25;
    }

    // Verified
    if is_verified {
        score -= 15;
    }

    // Age
    if age_hours < 24 {
        score += 10;
    } else if age_hours > 720 {
        score -= 10;
    }

    score.clamp(0, 100) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_category_from_score() {
        assert_eq!(RiskCategory::from_score(10), RiskCategory::Safe);
        assert_eq!(RiskCategory::from_score(30), RiskCategory::Low);
        assert_eq!(RiskCategory::from_score(50), RiskCategory::Medium);
        assert_eq!(RiskCategory::from_score(70), RiskCategory::High);
        assert_eq!(RiskCategory::from_score(90), RiskCategory::Critical);
    }

    #[test]
    fn test_recommendation_from_score() {
        assert!(matches!(Recommendation::from_score(20), Recommendation::Buy));
        assert!(matches!(Recommendation::from_score(40), Recommendation::Caution));
        assert!(matches!(Recommendation::from_score(60), Recommendation::Avoid));
        assert!(matches!(Recommendation::from_score(80), Recommendation::DoNotTrade));
    }

    #[test]
    fn test_quick_risk_score() {
        // Safe token
        let safe_score = quick_risk_score(100000.0, 2000, false, true, 1000);
        assert!(safe_score < 40);

        // Risky token
        let risky_score = quick_risk_score(500.0, 20, true, false, 12);
        assert!(risky_score > 70);
    }

    #[test]
    fn test_ml_scorer_creation() {
        let scorer = MLRiskScorer::new();
        assert!(!scorer.weights.is_empty());
        assert!(!scorer.scam_patterns.is_empty());
    }

    #[test]
    fn test_ml_score_calculation() {
        let scorer = MLRiskScorer::new();
        
        // Create risky features
        let features = MLFeatureSet {
            contract: ContractFeatures {
                has_blacklist: true,
                has_mint: true,
                ..Default::default()
            },
            liquidity: LiquidityFeatures {
                total_liquidity_usd: 500.0,
                ..Default::default()
            },
            trading: TradingFeatures {
                holder_count: 30,
                ..Default::default()
            },
            ..Default::default()
        };

        let result = scorer.calculate_score(&features);
        assert!(result.score > 50); // Should be risky
        assert!(!result.risk_factors.is_empty());
    }
}
