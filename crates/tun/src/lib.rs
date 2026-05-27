//! Platform-agnostic TUN device abstraction.
//!
//! The `TunDevice` trait provides a unified async read/write interface
//! for virtual network interfaces across platforms.  Backends are selected
//! at compile time via `#[cfg(target_os)]`.

use std::io;
use std::net::IpAddr;

use tokio::io::{AsyncRead, AsyncWrite};

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
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prefix_to_mask_v4() {
        let m = prefix_to_mask(24, false);
        assert_eq!(m, IpAddr::V4(std::net::Ipv4Addr::new(255, 255, 255, 0)));
    }

    #[test]
    fn test_prefix_to_mask_v6() {
        let m = prefix_to_mask(64, true);
        let expected = std::net::Ipv6Addr::new(0xffff, 0xffff, 0xffff, 0xffff, 0, 0, 0, 0);
        assert_eq!(m, IpAddr::V6(expected));
    }
}

pub fn create(name: Option<&str>) -> io::Result<impl TunDevice> {
    #[cfg(target_os = "linux")]
    { return linux::LinuxTun::create(name); }
    #[cfg(target_os = "macos")]
    { return macos::Utun::create(name); }
    #[cfg(target_os = "windows")]
    { return windows::WindowsTun::create(name); }
    #[allow(unreachable_code)]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "TUN is not yet supported on this platform",
        ))
    }
}
