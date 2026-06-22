//! Platform-agnostic TUN device abstraction.
//!
//! The `TunDevice` trait provides a unified async read/write interface
//! for virtual network interfaces across platforms.  Backends are selected
//! at compile time via `#[cfg(target_os)]`.

use std::io;
use std::net::IpAddr;

use tokio::io::{AsyncRead, AsyncWrite};

type TunPacketSender = tokio::sync::mpsc::Sender<Vec<u8>>;
type TunPacketReceiver = tokio::sync::mpsc::Receiver<Vec<u8>>;

// ── Address helpers ───────────────────────────────────────────────────

/// Convert an IP + prefix to a netmask.
pub fn prefix_to_mask(prefix: u8, v6: bool) -> IpAddr {
    if v6 {
        let mask = u128::MAX
            .checked_shl(128u32.saturating_sub(prefix as u32))
            .unwrap_or(0);
        IpAddr::V6(std::net::Ipv6Addr::from(mask.to_be_bytes()))
    } else {
        let mask = u32::MAX
            .checked_shl(32u32.saturating_sub(prefix as u32))
            .unwrap_or(0);
        IpAddr::V4(std::net::Ipv4Addr::from(mask.to_be_bytes()))
    }
}

// ── Trait ─────────────────────────────────────────────────────────────

/// An async virtual network interface.
///
/// Reads produce raw IP packets (IPv4 or IPv6).  Writes send packets
/// back into the tunnel.
#[allow(async_fn_in_trait)]
pub trait TunDevice: AsyncRead + AsyncWrite + Send + Sync + Unpin {
    /// Bring the interface up with the given address, netmask, and MTU.
    fn configure(&self, addr: IpAddr, mask: IpAddr, mtu: u16) -> io::Result<()>;

    /// Return the interface name (e.g. "utun8", "tun0").
    fn name(&self) -> &str;

    /// Consume the device and return mpsc channel endpoints for
    /// reading and writing raw IP packets.
    ///
    /// Used when the OS owns the TUN lifecycle (iOS `NEPacketTunnelProvider`,
    /// Android `VpnService`) and the application only sees packet channels.
    /// Default implementation bridges `AsyncRead`/`AsyncWrite` via spawned tasks.
    fn into_channels(self) -> io::Result<(TunPacketSender, TunPacketReceiver)>
    where
        Self: Sized + 'static,
    {
        let (read_tx, read_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(256);
        let (write_tx, mut write_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(256);

        let dev = std::sync::Arc::new(tokio::sync::Mutex::new(self));

        // Reader: TUN → channel
        let r = dev.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 65536];
            loop {
                let n = {
                    let mut d = r.lock().await;
                    tokio::io::AsyncReadExt::read(&mut *d, &mut buf).await
                };
                match n {
                    Ok(0) => break,
                    Ok(n) => {
                        if read_tx.send(buf[..n].to_vec()).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        // Writer: channel → TUN
        tokio::spawn(async move {
            while let Some(pkt) = write_rx.recv().await {
                let _ = tokio::io::AsyncWriteExt::write(&mut *dev.lock().await, &pkt).await;
            }
        });

        Ok((write_tx, read_rx))
    }
}

// ── Platform backends ─────────────────────────────────────────────────

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::LinuxTun;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::Utun;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::WindowsTun;

/// Create a new TUN device for the current platform.
///
/// Returns `None` if the platform is not yet supported.
pub fn create(name: Option<&str>) -> io::Result<impl TunDevice> {
    #[cfg(target_os = "linux")]
    {
        return linux::LinuxTun::create(name);
    }
    #[cfg(target_os = "macos")]
    {
        return macos::Utun::create(name);
    }
    #[cfg(target_os = "windows")]
    {
        return windows::WindowsTun::create(name);
    }
    #[allow(unreachable_code)]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "TUN is not yet supported on this platform",
        ))
    }
}
