//! Windows TUN device via Wintun.
//!
//! Uses WireGuard's Wintun driver for virtual network interfaces.
//! I/O is bridged from synchronous Wintun sessions to async tokio
//! via mpsc channels.
//!
//! ## wintun.dll resolution order
//!
//! 1. Binary-adjacent `wintun.dll` (same directory as the executable)
//! 2. `PATH` / system library search
//!
//! To bundle: place `wintun.dll` (from <https://wintun.net>) next to
//! the `zero` binary.  Release builds should ship it alongside.

use std::io;
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::mpsc;

use crate::TunDevice;

/// A Windows TUN device backed by Wintun.
///
/// Reads from a receiver filled by a background Wintun reader thread;
/// writes go to a sender consumed by a background Wintun writer thread.
pub struct WindowsTun {
    name: String,
    rx: mpsc::Receiver<Vec<u8>>,
    tx: mpsc::Sender<Vec<u8>>,
    _session: Arc<wintun::Session>,
    _adapter: Arc<wintun::Adapter>,
}

impl WindowsTun {
    /// Create a new Wintun TUN adapter.  `name` is the adapter name
    /// (e.g. `"ZeroTun"`).  The Wintun DLL must be available on the
    /// system (bundled with the binary or in `PATH`).
    pub fn create(name: Option<&str>) -> io::Result<Self> {
        let wintun = load_wintun()?;

        let adapter_name = name.unwrap_or("ZeroTun");
        let guid: u128 = 0xB6F4C8A2_1E3D_4F5A_9C2B_8D7E6A5F4C3B;

        let adapter = wintun::Adapter::create(&wintun, adapter_name, "ZeroTun", Some(guid))
            .map_err(|e| io::Error::other(format!("wintun create adapter: {e}")))?;

        let session = Arc::new(
            adapter
                .start_session(wintun::MAX_RING_CAPACITY)
                .map_err(|e| io::Error::other(format!("wintun start session: {e}")))?,
        );

        // Bridge Wintun (sync) ↔ tokio (async) via channels.
        let (read_tx, read_rx) = mpsc::channel::<Vec<u8>>(256);
        let (write_tx, mut write_rx) = mpsc::channel::<Vec<u8>>(256);

        // Reader thread.
        let reader_session = session.clone();
        std::thread::spawn(move || {
            while let Ok(pkt) = reader_session.receive_blocking() {
                let data = pkt.bytes().to_vec();
                if read_tx.blocking_send(data).is_err() {
                    break; // channel closed
                }
            }
        });

        // Writer thread.
        let writer_session = session.clone();
        std::thread::spawn(move || {
            while let Some(data) = write_rx.blocking_recv() {
                let len = data.len().min(u16::MAX as usize) as u16;
                match writer_session.allocate_send_packet(len) {
                    Ok(mut pkt) => {
                        pkt.bytes_mut()[..len as usize].copy_from_slice(&data[..len as usize]);
                        writer_session.send_packet(pkt);
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            name: adapter_name.to_owned(),
            rx: read_rx,
            tx: write_tx,
            _session: session,
            _adapter: adapter,
        })
    }
}

/// Load wintun.dll, trying binary-adjacent first, then system path.
fn load_wintun() -> io::Result<wintun::Wintun> {
    // 1. Try binary-adjacent `wintun.dll`.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let adjacent = dir.join("wintun.dll");
            if adjacent.exists() {
                return unsafe { wintun::load_from_path(&adjacent) }.map_err(|e| {
                    io::Error::other(format!(
                        "wintun load from {} failed: {e}",
                        adjacent.display()
                    ))
                });
            }
        }
    }

    // 2. Fall back to system PATH.
    unsafe { wintun::load() }.map_err(|_| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "wintun.dll not found\n\
             \n\
             Download from https://wintun.net and place wintun.dll:\n\
               • next to zero.exe (binary-adjacent), or\n\
               • anywhere in %PATH%\n\
             \n\
             On Linux/macOS: TUN works without extra drivers.",
        )
    })
}

impl AsyncRead for WindowsTun {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.rx.poll_recv(cx) {
            Poll::Ready(Some(data)) => {
                let n = data.len().min(buf.remaining());
                buf.put_slice(&data[..n]);
                Poll::Ready(Ok(()))
            }
            Poll::Ready(None) => Poll::Ready(Ok(())),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl AsyncWrite for WindowsTun {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.tx.try_send(buf.to_vec()) {
            Ok(()) => Poll::Ready(Ok(buf.len())),
            Err(mpsc::error::TrySendError::Full(_)) => Poll::Pending,
            Err(mpsc::error::TrySendError::Closed(_)) => {
                Poll::Ready(Err(io::Error::new(io::ErrorKind::BrokenPipe, "tun closed")))
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

impl TunDevice for WindowsTun {
    fn configure(&self, _addr: IpAddr, _mask: IpAddr, _mtu: u16) -> io::Result<()> {
        Ok(())
    }
    fn name(&self) -> &str {
        &self.name
    }
}
