use std::io;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use zero_platform_tokio::TokioSocket;

pub async fn relay_bidirectional_metered<F1, F2>(
    left: TokioSocket,
    right: TokioSocket,
    left_to_right: F1,
    right_to_left: F2,
) -> io::Result<(u64, u64)>
where
    F1: FnMut(u64),
    F2: FnMut(u64),
{
    let (left_read, left_write) = left.into_inner().into_split();
    let (right_read, right_write) = right.into_inner().into_split();

    tokio::try_join!(
        copy_one_way(left_read, right_write, left_to_right),
        copy_one_way(right_read, left_write, right_to_left)
    )
}

async fn copy_one_way<F>(
    mut reader: OwnedReadHalf,
    mut writer: OwnedWriteHalf,
    mut on_bytes: F,
) -> io::Result<u64>
where
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

        let read = read as u64;
        total = total.saturating_add(read);
        on_bytes(read);
    }
}

async fn shutdown_writer(writer: &mut OwnedWriteHalf) -> io::Result<()> {
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
