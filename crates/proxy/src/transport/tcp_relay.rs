use std::io;
use std::time::Instant;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::sleep;

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
    relay_bidirectional_metered_throttled(left, right, left_to_right, right_to_left, None, None).await
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
        copy_one_way(left_read, right_write, left_to_right, up_bps),
        copy_one_way(right_read, left_write, right_to_left, down_bps)
    )
}

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
    let mut buf = [0_u8; 16 * 1024];
    let mut total = 0_u64;
    let start = Instant::now();

    loop {
        let read = reader.read(&mut buf).await?;
        if read == 0 {
            shutdown_writer(&mut writer).await?;
            return Ok(total);
        }

        writer.write_all(&buf[..read]).await?;
        writer.flush().await?;

        let read = read as u64;
        total = total.saturating_add(read);
        on_bytes(read);

        // Throttle: sleep if ahead of the allowed rate.
        if let Some(bps) = rate_bps {
            if bps > 0 {
                let elapsed = start.elapsed();
                let allowed = (elapsed.as_secs_f64() * bps as f64) as u64;
                if total > allowed {
                    let deficit = total - allowed;
                    let wait =
                        std::time::Duration::from_secs_f64(deficit as f64 / bps as f64);
                    if wait > std::time::Duration::from_millis(1) {
                        sleep(wait).await;
                    }
                }
            }
        }
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
