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
use tracing::{debug, info, warn};

/// Rate limiter configuration
pub struct RateLimitConfig {
    /// Requests per window
    pub requests_per_window: u32,
    /// Window duration
    pub window_duration: Duration,
    /// Cleanup interval (remove expired entries)
    pub cleanup_interval: Duration,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_window: 100,                   // 100 requests
            window_duration: Duration::from_secs(60),   // per minute
            cleanup_interval: Duration::from_secs(300), // cleanup every 5 minutes
        }
    }
}

/// In-memory rate limiter with automatic cleanup
/// Production: Use Redis for distributed rate limiting
pub struct RateLimiter {
    /// Request counts per IP/API key
    requests: DashMap<String, (u32, Instant)>,
    config: RateLimitConfig,
    /// Last cleanup timestamp
    last_cleanup: std::sync::RwLock<Instant>,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            requests: DashMap::new(),
            last_cleanup: std::sync::RwLock::new(Instant::now()),
            config,
        }
    }

    /// Check if request is allowed, returns (allowed, remaining, reset_seconds)
    pub fn check(&self, key: &str) -> (bool, u32, u64) {
        let now = Instant::now();

        // Trigger cleanup if needed (non-blocking check)
        self.maybe_cleanup(now);

        let mut entry = self.requests.entry(key.to_string()).or_insert((0, now));

        // Reset window if expired
        if now.duration_since(entry.1) > self.config.window_duration {
            entry.0 = 0;
            entry.1 = now;
        }

        let remaining = self.config.requests_per_window.saturating_sub(entry.0);
        let reset_secs = self
            .config
            .window_duration
            .saturating_sub(now.duration_since(entry.1))
            .as_secs();

        if entry.0 >= self.config.requests_per_window {
            return (false, 0, reset_secs);
        }

        entry.0 += 1;
        (true, remaining - 1, reset_secs)
    }

    /// Check if cleanup is needed and perform it
    fn maybe_cleanup(&self, now: Instant) {
        // Quick read check first (non-blocking)
        let should_cleanup = {
            if let Ok(last) = self.last_cleanup.read() {
                now.duration_since(*last) > self.config.cleanup_interval
            } else {
                false
            }
        };

        if should_cleanup {
            // Try to acquire write lock (non-blocking)
            if let Ok(mut last) = self.last_cleanup.try_write() {
                // Double-check after acquiring lock
                if now.duration_since(*last) > self.config.cleanup_interval {
                    self.cleanup_expired_entries(now);
                    *last = now;
                }
            }
        }
    }

    /// Remove expired entries to prevent memory growth
    fn cleanup_expired_entries(&self, now: Instant) {
        let before_count = self.requests.len();
        let ttl = self.config.window_duration * 2; // Keep entries for 2x window duration

        self.requests
            .retain(|_, (_, timestamp)| now.duration_since(*timestamp) < ttl);

        let removed = before_count - self.requests.len();
        if removed > 0 {
            debug!(
                removed = removed,
                remaining = self.requests.len(),
                "Rate limiter cleanup completed"
            );
        }
    }

    /// Get current number of tracked keys (for monitoring)
    pub fn tracked_keys_count(&self) -> usize {
        self.requests.len()
    }

    /// Force cleanup (for manual trigger or shutdown)
    pub fn force_cleanup(&self) {
        self.cleanup_expired_entries(Instant::now());
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

/// Start background cleanup task (call once at server startup)
pub fn start_cleanup_task() {
    tokio::spawn(async {
        let mut interval = tokio::time::interval(Duration::from_secs(300)); // Every 5 minutes
        loop {
            interval.tick().await;
            RATE_LIMITER.force_cleanup();
            debug!(
                tracked_keys = RATE_LIMITER.tracked_keys_count(),
                "Periodic rate limiter cleanup"
            );
        }
    });
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
        Some(key) if validate_api_key(key) => Ok(next.run(request).await),
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
pub async fn logging_middleware(request: Request, next: Next) -> Response {
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
