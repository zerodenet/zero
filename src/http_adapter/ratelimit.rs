use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// A simple sliding-window rate limiter.
///
/// Tracks the number of requests in the current second.  When the second
/// rolls over the counter resets.  This is intentionally coarse — fine
/// enough for a local control API and cheap enough to check on every
/// request (two atomic ops).
pub struct RateLimiter {
    window_start: AtomicU64, // unix seconds
    count: AtomicU64,
    max_per_second: u64,
}

impl RateLimiter {
    pub fn new(max_per_second: u64) -> Self {
        Self {
            window_start: AtomicU64::new(now_secs()),
            count: AtomicU64::new(0),
            max_per_second,
        }
    }

    /// Try to acquire one permit.  Returns `true` if allowed.
    pub fn allow(&self) -> bool {
        let now = now_secs();
        let window = self.window_start.load(Ordering::Relaxed);

        if now != window {
            // New second — try to reset the window.
            if self
                .window_start
                .compare_exchange(window, now, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                self.count.store(1, Ordering::Release);
                return true;
            }
            // Someone else won the race; fall through to normal path.
        }

        let current = self.count.fetch_add(1, Ordering::Acquire);
        if current < self.max_per_second {
            return true;
        }

        // Over limit — undo the increment.
        self.count.fetch_sub(1, Ordering::Release);
        false
    }
}

/// Per-endpoint-category rate limiter group.
pub struct ApiRateLimiters {
    pub query: RateLimiter,
    pub command: RateLimiter,
    pub sse_connections: RateLimiter,
}

impl Default for ApiRateLimiters {
    fn default() -> Self {
        Self {
            query: RateLimiter::new(100),  // 100 req/s
            command: RateLimiter::new(10), // 10 req/s
            sse_connections: RateLimiter::new(5), // 5 concurrent
        }
    }
}

impl ApiRateLimiters {
    /// Disable a limiter by setting its cap to u64::MAX.
    #[allow(dead_code)]
    pub fn disable_query_limit(&mut self) {
        self.query.max_per_second = u64::MAX;
    }
    #[allow(dead_code)]
    pub fn disable_command_limit(&mut self) {
        self.command.max_per_second = u64::MAX;
    }
    #[allow(dead_code)]
    pub fn disable_sse_limit(&mut self) {
        self.sse_connections.max_per_second = u64::MAX;
    }
}

fn now_secs() -> u64 {
    // Using Instant for the window boundary is fine — we just need a
    // monotonically increasing value.
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let start = START.get_or_init(Instant::now);
    start.elapsed().as_secs()
}
