//! Core Module - Business Logic & REVM Engine
//!
//! Otak aplikasi: REVM simulation, honeypot detection, risk scoring.
//! CEO Directive: Logika inti tidak boleh berubah, hanya dipindahkan.
//!
//! ML Risk Scoring: Advanced weighted feature analysis for honeypot detection.

pub mod analyzer;
pub mod honeypot;
pub mod ml_risk;
pub mod risk_score;
pub mod simulator;

pub use analyzer::*;
pub use honeypot::*;
pub use ml_risk::*;
pub use risk_score::*;
pub use simulator::*;
