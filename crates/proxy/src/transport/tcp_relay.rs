use std::io;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub async fn relay_bidirectional_metered<L, R, F1, F2>(
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
    let (left_read, left_write) = tokio::io::split(left);
    let (right_read, right_write) = tokio::io::split(right);

    tokio::try_join!(
        copy_one_way(left_read, right_write, left_to_right),
        copy_one_way(right_read, left_write, right_to_left)
    )
}

async fn copy_one_way<R, W, F>(mut reader: R, mut writer: W, mut on_bytes: F) -> io::Result<u64>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
    F: FnMut(u64),
{
    let mut buf = [0_u8; 16 * 1024];
    let mut total = 0_u64;

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
