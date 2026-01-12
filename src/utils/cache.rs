//! High-Performance In-Memory Cache Module
//! 
//! Thread-safe caching layer untuk honeypot detection results.
//! Menggunakan DashMap untuk concurrent access tanpa lock contention.
//! 
//! Features:
//! - TTL-based expiration (5 menit default)
//! - Address normalization (lowercase)
//! - Cache HIT/MISS logging
//! - Thread-safe dengan DashMap

use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{info, debug};

use crate::core::honeypot::HoneypotResult;

/// Default TTL: 5 menit (300 detik)
const DEFAULT_TTL_SECS: u64 = 300;

/// Cache entry dengan timestamp untuk TTL validation
#[derive(Clone, Debug)]
pub struct CacheEntry {
    /// Hasil analisis honeypot
    pub result: HoneypotResult,
    /// Waktu saat entry dibuat
    pub created_at: Instant,
    /// TTL dalam detik
    pub ttl_secs: u64,
}

impl CacheEntry {
    /// Buat entry baru dengan TTL default
    pub fn new(result: HoneypotResult) -> Self {
        Self {
            result,
            created_at: Instant::now(),
            ttl_secs: DEFAULT_TTL_SECS,
        }
    }

    /// Buat entry dengan custom TTL
    #[allow(dead_code)]
    pub fn with_ttl(result: HoneypotResult, ttl_secs: u64) -> Self {
        Self {
            result,
            created_at: Instant::now(),
            ttl_secs,
        }
    }

    /// Cek apakah entry sudah expired
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > Duration::from_secs(self.ttl_secs)
    }

    /// Sisa waktu sebelum expired (dalam detik)
    #[allow(dead_code)]
    pub fn remaining_ttl(&self) -> u64 {
        let elapsed = self.created_at.elapsed().as_secs();
        self.ttl_secs.saturating_sub(elapsed)
    }
}

/// Global Cache State menggunakan DashMap
/// Thread-safe tanpa explicit locking
#[derive(Clone)]
pub struct HoneypotCache {
    /// Internal storage: lowercase address -> CacheEntry
    store: Arc<DashMap<String, CacheEntry>>,
    /// TTL dalam detik
    ttl_secs: u64,
    /// Counter untuk statistik
    hits: Arc<std::sync::atomic::AtomicU64>,
    misses: Arc<std::sync::atomic::AtomicU64>,
}

impl Default for HoneypotCache {
    fn default() -> Self {
        Self::new()
    }
}

impl HoneypotCache {
    /// Buat cache baru dengan TTL default (5 menit)
    pub fn new() -> Self {
        Self {
            store: Arc::new(DashMap::new()),
            ttl_secs: DEFAULT_TTL_SECS,
            hits: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            misses: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    /// Buat cache dengan custom TTL
    #[allow(dead_code)]
    pub fn with_ttl(ttl_secs: u64) -> Self {
        Self {
            store: Arc::new(DashMap::new()),
            ttl_secs,
            hits: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            misses: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    /// Normalisasi address ke lowercase
    #[inline]
    fn normalize_address(address: &str) -> String {
        address.to_lowercase()
    }

    /// Get dari cache dengan TTL validation
    /// Returns Some(result) jika cache HIT dan belum expired
    /// Returns None jika cache MISS atau expired
    pub fn get(&self, address: &str) -> Option<HoneypotResult> {
        let key = Self::normalize_address(address);
        
        if let Some(entry) = self.store.get(&key) {
            if entry.is_expired() {
                // Entry expired, hapus dan return None
                drop(entry); // Release read lock
                self.store.remove(&key);
                self.misses.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                debug!("📭 CACHE MISS (expired): {}", key);
                None
            } else {
                // Cache HIT!
                self.hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let remaining = entry.remaining_ttl();
                info!("✅ CACHE HIT: {} (TTL: {}s remaining)", key, remaining);
                Some(entry.result.clone())
            }
        } else {
            // Cache MISS
            self.misses.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            debug!("📭 CACHE MISS: {}", key);
            None
        }
    }

    /// Set ke cache dengan TTL default
    /// Hanya simpan hasil yang valid (bukan error)
    pub fn set(&self, address: &str, result: HoneypotResult) {
        let key = Self::normalize_address(address);
        let entry = CacheEntry {
            result,
            created_at: Instant::now(),
            ttl_secs: self.ttl_secs,
        };
        
        self.store.insert(key.clone(), entry);
        info!("💾 CACHE SET: {} (TTL: {}s)", key, self.ttl_secs);
    }

    /// Hapus entry dari cache
    #[allow(dead_code)]
    pub fn invalidate(&self, address: &str) {
        let key = Self::normalize_address(address);
        self.store.remove(&key);
        debug!("🗑️ CACHE INVALIDATE: {}", key);
    }

    /// Bersihkan semua entry yang expired
    #[allow(dead_code)]
    pub fn cleanup_expired(&self) -> usize {
        let before = self.store.len();
        self.store.retain(|_, entry| !entry.is_expired());
        let removed = before - self.store.len();
        if removed > 0 {
            info!("🧹 CACHE CLEANUP: {} expired entries removed", removed);
        }
        removed
    }

    /// Get statistik cache
    pub fn stats(&self) -> CacheStats {
        let hits = self.hits.load(std::sync::atomic::Ordering::Relaxed);
        let misses = self.misses.load(std::sync::atomic::Ordering::Relaxed);
        let total = hits + misses;
        let hit_rate = if total > 0 {
            (hits as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        CacheStats {
            entries: self.store.len(),
            hits,
            misses,
            hit_rate,
            ttl_secs: self.ttl_secs,
        }
    }

    /// Clear semua cache
    #[allow(dead_code)]
    pub fn clear(&self) {
        self.store.clear();
        info!("🗑️ CACHE CLEARED");
    }
}

/// Statistik cache untuk monitoring
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub entries: usize,
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
    pub ttl_secs: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_result() -> HoneypotResult {
        HoneypotResult::safe(1.0, 1.0, 0, vec![], 100)
    }

    #[test]
    fn test_cache_set_get() {
        let cache = HoneypotCache::new();
        let address = "0xdAC17F958D2ee523a2206206994597C13D831ec7";
        
        // Set
        cache.set(address, mock_result());
        
        // Get - should hit
        let result = cache.get(address);
        assert!(result.is_some());
    }

    #[test]
    fn test_address_normalization() {
        let cache = HoneypotCache::new();
        
        // Set dengan uppercase
        cache.set("0xDAC17F958D2EE523A2206206994597C13D831EC7", mock_result());
        
        // Get dengan lowercase - should hit
        let result = cache.get("0xdac17f958d2ee523a2206206994597c13d831ec7");
        assert!(result.is_some());
    }

    #[test]
    fn test_cache_miss() {
        let cache = HoneypotCache::new();
        let result = cache.get("0x1234567890123456789012345678901234567890");
        assert!(result.is_none());
    }

    #[test]
    fn test_cache_stats() {
        let cache = HoneypotCache::new();
        let address = "0xtest";
        
        cache.set(address, mock_result());
        cache.get(address); // HIT
        cache.get("0xnonexistent"); // MISS
        
        let stats = cache.stats();
        assert_eq!(stats.entries, 1);
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
    }
}
