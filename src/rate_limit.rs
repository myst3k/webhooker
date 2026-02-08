use std::net::IpAddr;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use uuid::Uuid;

/// Per-IP-per-endpoint submission rate limiter using sliding window.
pub struct SubmissionRateLimiter {
    /// (endpoint_id, ip) -> (count, window_start)
    entries: DashMap<(Uuid, IpAddr), (u32, Instant)>,
}

impl SubmissionRateLimiter {
    pub fn new() -> Self {
        Self {
            entries: DashMap::new(),
        }
    }

    /// Check if request is allowed. Returns Ok(()) or Err with retry-after seconds.
    pub fn check(&self, endpoint_id: Uuid, ip: IpAddr, limit: u32, window_secs: u64) -> Result<(), u64> {
        let key = (endpoint_id, ip);
        let window = Duration::from_secs(window_secs);
        let now = Instant::now();

        let mut entry = self.entries.entry(key).or_insert((0, now));
        let (count, start) = entry.value_mut();

        if now.duration_since(*start) > window {
            *count = 1;
            *start = now;
            return Ok(());
        }

        if *count >= limit {
            let elapsed = now.duration_since(*start).as_secs();
            return Err(window_secs.saturating_sub(elapsed));
        }

        *count += 1;
        Ok(())
    }

    /// Remove stale entries older than the given duration.
    pub fn cleanup(&self, max_age: Duration) {
        let now = Instant::now();
        self.entries.retain(|_, (_, start)| now.duration_since(*start) < max_age);
    }
}

/// Per-email login brute force limiter.
pub struct LoginRateLimiter {
    /// email -> (failed_count, window_start)
    entries: DashMap<String, (u32, Instant)>,
}

impl LoginRateLimiter {
    pub fn new() -> Self {
        Self {
            entries: DashMap::new(),
        }
    }

    /// Check if login attempt is allowed. 5 attempts per 15 minutes.
    pub fn check(&self, email: &str) -> Result<(), u64> {
        let window = Duration::from_secs(15 * 60);
        let now = Instant::now();

        let mut entry = self.entries.entry(email.to_lowercase()).or_insert((0, now));
        let (count, start) = entry.value_mut();

        if now.duration_since(*start) > window {
            *count = 1;
            *start = now;
            return Ok(());
        }

        if *count >= 5 {
            let elapsed = now.duration_since(*start).as_secs();
            return Err((15 * 60u64).saturating_sub(elapsed));
        }

        *count += 1;
        Ok(())
    }

    /// Record a failed attempt without checking (call after check passes but login fails).
    pub fn record_failure(&self, email: &str) {
        // The check already incremented, so nothing extra needed.
        // This method exists for clarity at call sites.
        let _ = email;
    }

    pub fn cleanup(&self, max_age: Duration) {
        let now = Instant::now();
        self.entries.retain(|_, (_, start)| now.duration_since(*start) < max_age);
    }
}
