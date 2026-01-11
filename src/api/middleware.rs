//! API Middleware (Auth, Rate Limiting, Logging)

use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{info, warn};

/// Rate limiter configuration
pub struct RateLimitConfig {
    /// Requests per window
    pub requests_per_window: u32,
    /// Window duration
    pub window_duration: Duration,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_window: 100,  // 100 requests
            window_duration: Duration::from_secs(60), // per minute
        }
    }
}

/// In-memory rate limiter
/// Production: Use Redis for distributed rate limiting
pub struct RateLimiter {
    /// Request counts per IP/API key
    requests: DashMap<String, (u32, Instant)>,
    config: RateLimitConfig,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            requests: DashMap::new(),
            config,
        }
    }
    
    /// Check if request is allowed, returns (allowed, remaining, reset_seconds)
    pub fn check(&self, key: &str) -> (bool, u32, u64) {
        let now = Instant::now();
        
        let mut entry = self.requests.entry(key.to_string()).or_insert((0, now));
        
        // Reset window if expired
        if now.duration_since(entry.1) > self.config.window_duration {
            entry.0 = 0;
            entry.1 = now;
        }
        
        let remaining = self.config.requests_per_window.saturating_sub(entry.0);
        let reset_secs = self.config.window_duration
            .saturating_sub(now.duration_since(entry.1))
            .as_secs();
        
        if entry.0 >= self.config.requests_per_window {
            return (false, 0, reset_secs);
        }
        
        entry.0 += 1;
        (true, remaining - 1, reset_secs)
    }
    
    /// Cleanup old entries (call periodically)
    #[allow(dead_code)]
    pub fn cleanup(&self) {
        let now = Instant::now();
        self.requests.retain(|_, (_, timestamp)| {
            now.duration_since(*timestamp) < self.config.window_duration * 2
        });
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(RateLimitConfig::default())
    }
}

// Global rate limiter instance
lazy_static::lazy_static! {
    pub static ref RATE_LIMITER: Arc<RateLimiter> = Arc::new(RateLimiter::default());
}

/// API Key authentication middleware
pub async fn auth_middleware(
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Skip auth for health check
    if request.uri().path() == "/health" || request.uri().path() == "/v1/health" {
        return Ok(next.run(request).await);
    }
    
    // Check for API key
    let api_key = headers
        .get("X-API-Key")
        .or_else(|| headers.get("x-api-key"))
        .and_then(|v| v.to_str().ok());
    
    match api_key {
        Some(key) if validate_api_key(key) => {
            Ok(next.run(request).await)
        }
        Some(_) => {
            warn!("Invalid API key attempted");
            Err(StatusCode::UNAUTHORIZED)
        }
        None => {
            // For MVP, allow requests without API key (rate limited)
            Ok(next.run(request).await)
        }
    }
}

/// Validate API key format and existence
fn validate_api_key(key: &str) -> bool {
    // MVP: Accept any key starting with "sk_" or "pk_"
    // Production: Check against database/Redis
    key.starts_with("sk_") || key.starts_with("pk_") || key == "demo"
}

/// Rate limiting middleware
pub async fn rate_limit_middleware(
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Skip rate limiting for health check
    if request.uri().path() == "/health" || request.uri().path() == "/v1/health" {
        return Ok(next.run(request).await);
    }
    
    // Get rate limit key (API key or IP)
    let rate_key = headers
        .get("X-API-Key")
        .or_else(|| headers.get("x-api-key"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            // Fallback to IP-based limiting
            headers
                .get("X-Forwarded-For")
                .or_else(|| headers.get("x-real-ip"))
                .and_then(|v| v.to_str().ok())
                .unwrap_or("unknown")
                .to_string()
        });
    
    let (allowed, remaining, reset) = RATE_LIMITER.check(&rate_key);
    
    if !allowed {
        warn!(key = %rate_key, "Rate limit exceeded");
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    
    let mut response = next.run(request).await;
    
    // Add rate limit headers
    let headers = response.headers_mut();
    headers.insert("X-RateLimit-Remaining", remaining.into());
    headers.insert("X-RateLimit-Reset", reset.into());
    
    Ok(response)
}

/// Request logging middleware
pub async fn logging_middleware(
    request: Request,
    next: Next,
) -> Response {
    let start = Instant::now();
    let method = request.method().clone();
    let uri = request.uri().clone();
    
    let response = next.run(request).await;
    
    let latency = start.elapsed();
    let status = response.status();
    
    info!(
        method = %method,
        uri = %uri,
        status = %status.as_u16(),
        latency_ms = %latency.as_millis(),
        "Request completed"
    );
    
    response
}
