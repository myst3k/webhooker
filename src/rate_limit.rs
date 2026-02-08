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

    /// Check if login attempt is allowed. 5 failures per 15 minutes.
    /// Does NOT increment the counter — call `record_failure()` on invalid password.
    pub fn check(&self, email: &str) -> Result<(), u64> {
        let window = Duration::from_secs(15 * 60);
        let now = Instant::now();

        let entry = self.entries.get(&email.to_lowercase());
        let Some(entry) = entry else {
            return Ok(());
        };

        let (count, start) = entry.value();

        if now.duration_since(*start) > window {
            return Ok(());
        }

        if *count >= 5 {
            let elapsed = now.duration_since(*start).as_secs();
            return Err((15 * 60u64).saturating_sub(elapsed));
        }

        Ok(())
    }

    /// Record a failed login attempt. Increments the counter for the given email.
    pub fn record_failure(&self, email: &str) {
        let window = Duration::from_secs(15 * 60);
        let now = Instant::now();

        let mut entry = self.entries.entry(email.to_lowercase()).or_insert((0, now));
        let (count, start) = entry.value_mut();

        if now.duration_since(*start) > window {
            *count = 1;
            *start = now;
        } else {
            *count += 1;
        }
    }

    pub fn cleanup(&self, max_age: Duration) {
        let now = Instant::now();
        self.entries.retain(|_, (_, start)| now.duration_since(*start) < max_age);
    }
}
