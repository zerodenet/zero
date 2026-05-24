use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

// ── GCRA rate limiter ─────────────────────────────────────────────────

/// Single-threaded GCRA (Generic Cell Rate Algorithm) limiter for byte streams.
///
/// Non-blocking — callers integrate via `AsyncWrite::poll_write`.
pub(crate) struct RateLimiter {
    /// Theoretical arrival time of the next byte.
    tat: Instant,
    /// Time allowance per byte (`1.0 / rate_bps` seconds).
    per_byte: Duration,
    /// Burst tolerance (avoids starving small writes).
    burst: Duration,
}

impl RateLimiter {
    pub(crate) fn new(rate_bps: u64) -> Self {
        let per_byte = Duration::from_secs_f64(1.0 / rate_bps as f64);
        let burst = per_byte.saturating_mul(16384); // one buffer of headroom
        Self {
            tat: Instant::now(),
            per_byte,
            burst,
        }
    }

    /// Try to consume `n` bytes.  Returns `Ok(())` if allowed immediately,
    /// or `Err(wait)` with the duration to wait before retrying.
    pub(crate) fn check_n(&mut self, n: u64) -> Result<(), Duration> {
        let now = Instant::now();
        let tat = self.tat.max(now);
        let emission = self.per_byte.saturating_mul(n as u32);
        let arrival = tat + emission;
        let deadline = arrival.checked_sub(self.burst).unwrap_or(arrival);
        if deadline <= now {
            self.tat = arrival;
            Ok(())
        } else {
            Err(deadline.duration_since(now))
        }
    }
}

// ── Rate-limited async writer ─────────────────────────────────────────

/// Wraps an `AsyncWrite`, applying GCRA rate limiting in `poll_write`.
///
/// When the rate limit is exceeded, `poll_write` registers a timer and
/// returns `Poll::Pending` — no `sleep` needed.  The tokio runtime wakes
/// the task when tokens become available.
pub(crate) struct RateLimitedWriter<W> {
    inner: W,
    limiter: RateLimiter,
    timer: Pin<Box<tokio::time::Sleep>>,
    timer_set: bool,
}

impl<W: AsyncWrite + Unpin> RateLimitedWriter<W> {
    pub(crate) fn new(inner: W, rate_bps: u64) -> Self {
        Self {
            inner,
            limiter: RateLimiter::new(rate_bps),
            timer: Box::pin(tokio::time::sleep(Duration::ZERO)),
            timer_set: false,
        }
    }
}

impl<W: AsyncWrite + Unpin> AsyncWrite for RateLimitedWriter<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        // Check rate limit — loop in case timer just fired.
        match self.limiter.check_n(buf.len() as u64) {
            Ok(()) => {
                self.timer_set = false;
                Pin::new(&mut self.inner).poll_write(cx, buf)
            }
            Err(wait) => {
                if !self.timer_set {
                    self.timer
                        .as_mut()
                        .reset(tokio::time::Instant::now() + wait);
                    self.timer_set = true;
                }
                // Poll the timer; when Ready, re-poll from the top.
                match Future::poll(self.timer.as_mut(), cx) {
                    Poll::Ready(()) => {
                        self.timer_set = false;
                        cx.waker().wake_by_ref();
                    }
                    Poll::Pending => {}
                }
                Poll::Pending
            }
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

// ── Bidirectional relay ───────────────────────────────────────────────

pub(crate) async fn relay_bidirectional_metered<L, R, F1, F2>(
    left: L,
    right: R,
    left_to_right: F1,
    right_to_left: F2,
) -> io::Result<(u64, u64)>
where
    L: AsyncRead + AsyncWrite + Send + Unpin,
    R: AsyncRead + AsyncWrite + Send + Unpin,
    F1: FnMut(u64),
    F2: FnMut(u64),
{
    relay_bidirectional_metered_throttled(left, right, left_to_right, right_to_left, None, None)
        .await
}

/// Like [`relay_bidirectional_metered`] but with optional rate limiting.
///
/// `up_bps` limits left→right (client upload).
/// `down_bps` limits right→left (client download).
pub(crate) async fn relay_bidirectional_metered_throttled<L, R, F1, F2>(
    left: L,
    right: R,
    left_to_right: F1,
    right_to_left: F2,
    up_bps: Option<u64>,
    down_bps: Option<u64>,
) -> io::Result<(u64, u64)>
where
    L: AsyncRead + AsyncWrite + Send + Unpin,
    R: AsyncRead + AsyncWrite + Send + Unpin,
    F1: FnMut(u64),
    F2: FnMut(u64),
{
    let (left_read, left_write) = tokio::io::split(left);
    let (right_read, right_write) = tokio::io::split(right);

    tokio::try_join!(
        copy_one_way(left_read, left_write, left_to_right, up_bps),
        copy_one_way(right_read, right_write, right_to_left, down_bps)
    )
}

/// Uni-directional byte copy with optional rate limiting.
///
/// With a rate limit, writes go through a `RateLimitedWriter` whose
/// `poll_write` returns `Pending` when over rate — no `sleep`, no spinning.
pub(crate) async fn copy_one_way<R, W, F>(
    mut reader: R,
    mut writer: W,
    mut on_bytes: F,
    rate_bps: Option<u64>,
) -> io::Result<u64>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
    F: FnMut(u64),
{
    if let Some(bps) = rate_bps {
        if bps > 0 {
            let mut rl_writer = RateLimitedWriter::new(writer, bps);
            return copy_loop(&mut reader, &mut rl_writer, &mut on_bytes).await;
        }
    }
    copy_loop(&mut reader, &mut writer, &mut on_bytes).await
}

async fn copy_loop<R, W, F>(reader: &mut R, writer: &mut W, on_bytes: &mut F) -> io::Result<u64>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
    F: FnMut(u64),
{
    let mut buf = [0_u8; 16 * 1024];
    let mut total = 0_u64;

    loop {
        let n = reader.read(&mut buf).await?;
        if n == 0 {
            shutdown_writer(writer).await?;
            return Ok(total);
        }
        writer.write_all(&buf[..n]).await?;
        total = total.saturating_add(n as u64);
        on_bytes(n as u64);
    }
}

async fn shutdown_writer(writer: &mut (impl AsyncWrite + Unpin)) -> io::Result<()> {
    match writer.shutdown().await {
        Ok(()) => Ok(()),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotConnected | io::ErrorKind::BrokenPipe
            ) =>
        {
            Ok(())
        }
        Err(error) => Err(error),
    }
}
