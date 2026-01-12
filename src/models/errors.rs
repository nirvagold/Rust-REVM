//! Centralized Error Handling Module
//!
//! CEO Directive: Setiap kegagalan harus memiliki kode error yang unik.
//! Ini memudahkan debugging dan monitoring di production (Koyeb).
//!
//! Error codes follow pattern: CATEGORY_SPECIFIC_ERROR
//! - RPC_xxx: RPC-related errors
//! - SIM_xxx: Simulation errors
//! - API_xxx: API errors
//! - CFG_xxx: Configuration errors

use std::fmt;

/// Application-wide error type
/// CEO Directive: All errors must flow through this type
#[derive(Debug)]
pub struct AppError {
    /// Unique error code for logging/monitoring
    pub code: ErrorCode,
    /// Human-readable message
    pub message: String,
    /// Optional underlying error
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl AppError {
    /// Create a new AppError
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            source: None,
        }
    }

    /// Create AppError with source error
    pub fn with_source(
        code: ErrorCode,
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Get error code as string (for logging)
    pub fn code_str(&self) -> &'static str {
        self.code.as_str()
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_ref().map(|e| e.as_ref() as &(dyn std::error::Error + 'static))
    }
}

/// Unique error codes for monitoring
/// CEO Directive: Each error type has a unique code for Koyeb logs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    // ============================================
    // RPC Errors (1xx)
    // ============================================
    /// RPC connection failed
    RpcConnectionFailed,
    /// RPC request timeout
    RpcTimeout,
    /// RPC rate limited (HTTP 429)
    RpcRateLimited,
    /// RPC returned error response
    RpcError,
    /// No RPC endpoints available
    RpcNoEndpoints,
    /// Invalid RPC response
    RpcInvalidResponse,

    // ============================================
    // Simulation Errors (2xx)
    // ============================================
    /// Simulation reverted
    SimulationReverted,
    /// Simulation halted (out of gas, etc.)
    SimulationHalted,
    /// Simulation failed (generic)
    SimulationFailed,
    /// Buy simulation failed
    SimulationBuyFailed,
    /// Sell simulation failed (potential honeypot)
    SimulationSellFailed,
    /// Approve simulation failed
    SimulationApproveFailed,

    // ============================================
    // API Errors (3xx)
    // ============================================
    /// Invalid request format
    ApiBadRequest,
    /// Unauthorized (invalid API key)
    ApiUnauthorized,
    /// Rate limit exceeded
    ApiRateLimited,
    /// Internal server error
    ApiInternalError,
    /// Resource not found
    ApiNotFound,

    // ============================================
    // Configuration Errors (4xx)
    // ============================================
    /// Missing environment variable
    ConfigMissingEnv,
    /// Invalid configuration value
    ConfigInvalidValue,
    /// Unsupported chain ID
    ConfigUnsupportedChain,
    /// Missing API key
    ConfigMissingApiKey,

    // ============================================
    // Token/Contract Errors (5xx)
    // ============================================
    /// Invalid token address
    TokenInvalidAddress,
    /// Token not found (no liquidity)
    TokenNotFound,
    /// Token is honeypot
    TokenHoneypot,
    /// Token has high tax
    TokenHighTax,
    /// Contract not verified
    ContractNotVerified,

    // ============================================
    // External Service Errors (6xx)
    // ============================================
    /// DexScreener API error
    DexScreenerError,
    /// Alchemy API error
    AlchemyError,
    /// External service timeout
    ExternalTimeout,

    // ============================================
    // Generic Errors (9xx)
    // ============================================
    /// Unknown error
    Unknown,
}

impl ErrorCode {
    /// Get string representation of error code
    pub fn as_str(&self) -> &'static str {
        match self {
            // RPC Errors
            Self::RpcConnectionFailed => "RPC_CONNECTION_FAILED",
            Self::RpcTimeout => "RPC_TIMEOUT",
            Self::RpcRateLimited => "RPC_RATE_LIMITED",
            Self::RpcError => "RPC_ERROR",
            Self::RpcNoEndpoints => "RPC_NO_ENDPOINTS",
            Self::RpcInvalidResponse => "RPC_INVALID_RESPONSE",

            // Simulation Errors
            Self::SimulationReverted => "SIM_REVERTED",
            Self::SimulationHalted => "SIM_HALTED",
            Self::SimulationFailed => "SIM_FAILED",
            Self::SimulationBuyFailed => "SIM_BUY_FAILED",
            Self::SimulationSellFailed => "SIM_SELL_FAILED",
            Self::SimulationApproveFailed => "SIM_APPROVE_FAILED",

            // API Errors
            Self::ApiBadRequest => "API_BAD_REQUEST",
            Self::ApiUnauthorized => "API_UNAUTHORIZED",
            Self::ApiRateLimited => "API_RATE_LIMITED",
            Self::ApiInternalError => "API_INTERNAL_ERROR",
            Self::ApiNotFound => "API_NOT_FOUND",

            // Configuration Errors
            Self::ConfigMissingEnv => "CFG_MISSING_ENV",
            Self::ConfigInvalidValue => "CFG_INVALID_VALUE",
            Self::ConfigUnsupportedChain => "CFG_UNSUPPORTED_CHAIN",
            Self::ConfigMissingApiKey => "CFG_MISSING_API_KEY",

            // Token/Contract Errors
            Self::TokenInvalidAddress => "TOKEN_INVALID_ADDRESS",
            Self::TokenNotFound => "TOKEN_NOT_FOUND",
            Self::TokenHoneypot => "TOKEN_HONEYPOT",
            Self::TokenHighTax => "TOKEN_HIGH_TAX",
            Self::ContractNotVerified => "CONTRACT_NOT_VERIFIED",

            // External Service Errors
            Self::DexScreenerError => "DEXSCREENER_ERROR",
            Self::AlchemyError => "ALCHEMY_ERROR",
            Self::ExternalTimeout => "EXTERNAL_TIMEOUT",

            // Generic
            Self::Unknown => "UNKNOWN_ERROR",
        }
    }

    /// Get HTTP status code for API responses
    pub fn http_status(&self) -> u16 {
        match self {
            Self::ApiBadRequest | Self::TokenInvalidAddress | Self::ConfigInvalidValue => 400,
            Self::ApiUnauthorized | Self::ConfigMissingApiKey => 401,
            Self::ApiNotFound | Self::TokenNotFound => 404,
            Self::ApiRateLimited | Self::RpcRateLimited => 429,
            _ => 500,
        }
    }

    /// Check if error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::RpcTimeout
                | Self::RpcRateLimited
                | Self::RpcConnectionFailed
                | Self::ExternalTimeout
                | Self::DexScreenerError
        )
    }
}

// ============================================
// Convenience constructors
// ============================================

impl AppError {
    /// RPC connection failed
    pub fn rpc_connection_failed(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::RpcConnectionFailed, msg)
    }

    /// RPC timeout
    pub fn rpc_timeout(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::RpcTimeout, msg)
    }

    /// RPC rate limited
    pub fn rpc_rate_limited() -> Self {
        Self::new(ErrorCode::RpcRateLimited, "Rate limited (HTTP 429)")
    }

    /// Simulation failed
    pub fn simulation_failed(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::SimulationFailed, msg)
    }

    /// Simulation reverted
    pub fn simulation_reverted(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::SimulationReverted, msg)
    }

    /// Buy simulation failed
    pub fn buy_failed(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::SimulationBuyFailed, msg)
    }

    /// Sell simulation failed (potential honeypot)
    pub fn sell_failed(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::SimulationSellFailed, msg)
    }

    /// Invalid token address
    pub fn invalid_address(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::TokenInvalidAddress, msg)
    }

    /// Token not found
    pub fn token_not_found(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::TokenNotFound, msg)
    }

    /// Unsupported chain
    pub fn unsupported_chain(chain_id: u64) -> Self {
        Self::new(
            ErrorCode::ConfigUnsupportedChain,
            format!("Unsupported chain_id: {}", chain_id),
        )
    }

    /// Missing API key
    pub fn missing_api_key(key_name: &str) -> Self {
        Self::new(
            ErrorCode::ConfigMissingApiKey,
            format!("Missing API key: {}", key_name),
        )
    }

    /// DexScreener error
    pub fn dexscreener_error(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::DexScreenerError, msg)
    }

    /// API bad request
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::ApiBadRequest, msg)
    }

    /// API internal error
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::ApiInternalError, msg)
    }
}

// ============================================
// Result type alias
// ============================================

/// Application Result type
pub type AppResult<T> = Result<T, AppError>;

// ============================================
// Conversion from common error types
// ============================================

impl From<eyre::Report> for AppError {
    fn from(err: eyre::Report) -> Self {
        Self::new(ErrorCode::Unknown, err.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        Self::with_source(ErrorCode::Unknown, "IO error", err)
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            Self::new(ErrorCode::ExternalTimeout, "Request timeout")
        } else if err.is_connect() {
            Self::new(ErrorCode::RpcConnectionFailed, "Connection failed")
        } else {
            Self::new(ErrorCode::Unknown, err.to_string())
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        Self::with_source(ErrorCode::RpcInvalidResponse, "JSON parse error", err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        let err = AppError::rpc_timeout("Connection timed out");
        assert_eq!(err.code, ErrorCode::RpcTimeout);
        assert_eq!(err.code_str(), "RPC_TIMEOUT");
    }

    #[test]
    fn test_retryable() {
        assert!(ErrorCode::RpcTimeout.is_retryable());
        assert!(ErrorCode::RpcRateLimited.is_retryable());
        assert!(!ErrorCode::TokenHoneypot.is_retryable());
    }

    #[test]
    fn test_http_status() {
        assert_eq!(ErrorCode::ApiBadRequest.http_status(), 400);
        assert_eq!(ErrorCode::ApiRateLimited.http_status(), 429);
        assert_eq!(ErrorCode::SimulationFailed.http_status(), 500);
    }
}
