//! Core Module - Business Logic & REVM Engine
//!
//! Otak aplikasi: REVM simulation, honeypot detection, risk scoring.
//! CEO Directive: Logika inti tidak boleh berubah, hanya dipindahkan.

pub mod analyzer;
pub mod honeypot;
pub mod risk_score;
pub mod simulator;

pub use analyzer::*;
pub use honeypot::*;
pub use risk_score::*;
pub use simulator::*;
