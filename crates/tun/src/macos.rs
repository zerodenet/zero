//! macOS utun device via `SYSPROTO_CONTROL` socket.
//!
//! Creates a virtual network interface using XNU's built-in utun driver.
//! The socket provides raw IP packet I/O — no Ethernet header.
//!
//! Reference: <https://developer.apple.com/documentation/networkextension>

use std::ffi::CString;
use std::io;
use std::net::IpAddr;
use std::os::unix::io::{AsRawFd, RawFd};
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::unix::AsyncFd;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::TunDevice;

// XNU system controls
const SYSPROTO_CONTROL: libc::c_int = 2; // SYSPROTO_CONTROL
const AF_SYSTEM: libc::c_int = 32; // AF_SYSTEM
const CTLIOCGINFO: libc::c_ulong = 0xc064_4e03;
const UTUN_CONTROL_NAME: &str = "com.apple.net.utun_control";
const UTUN_OPT_IFNAME: libc::c_int = 2;

/// A macOS utun device backed by an `AsyncFd`.
pub struct Utun {
    name: String,
    fd: AsyncFd<RawFd>,
}

impl Utun {
    /// Create a new utun device.  `name` is ignored (the kernel assigns
    /// the interface index automatically).
    pub fn create(_name: Option<&str>) -> io::Result<Self> {
        // Find the utun control ID
        let ctl_id = find_utun_control()?;

        // Create system socket
        let sock = unsafe { libc::socket(AF_SYSTEM, libc::SOCK_DGRAM, SYSPROTO_CONTROL) };
        if sock < 0 {
            return Err(io::Error::last_os_error());
        }

        // Connect to utun control
        let mut ctl_info: libc::ctl_info = unsafe { std::mem::zeroed() };
        ctl_info.ctl_id = ctl_id;
        // SAFETY: u8→c_char cast, ASCII names only.
        let name_bytes = UTUN_CONTROL_NAME.as_bytes();
        let copy_len = name_bytes.len().min(ctl_info.ctl_name.len() - 1);
        for (i, &b) in name_bytes.iter().take(copy_len).enumerate() {
            ctl_info.ctl_name[i] = b as libc::c_char;
        }

        let ret = unsafe {
            libc::connect(
                sock,
                &ctl_info as *const _ as *const libc::sockaddr,
                std::mem::size_of::<libc::ctl_info>() as libc::socklen_t,
            )
        };
        if ret < 0 {
            unsafe { libc::close(sock) };
            return Err(io::Error::last_os_error());
        }

        // Get assigned interface name (utunN)
        let mut ifname: [libc::c_char; 16] = unsafe { std::mem::zeroed() };
        let mut ifname_len: libc::socklen_t = ifname.len() as libc::socklen_t;
        let ret = unsafe {
            libc::getsockopt(
                sock,
                SYSPROTO_CONTROL,
                UTUN_OPT_IFNAME,
                ifname.as_mut_ptr() as *mut libc::c_void,
                &mut ifname_len,
            )
        };
        if ret < 0 {
            unsafe { libc::close(sock) };
            return Err(io::Error::last_os_error());
        }

        let name = ifname
            .iter()
            .take_while(|&&b| b != 0)
            .map(|&b| b as u8 as char)
            .collect::<String>();

        // Set non-blocking
        unsafe {
            let flags = libc::fcntl(sock, libc::F_GETFL, 0);
            if flags >= 0 {
                libc::fcntl(sock, libc::F_SETFL, flags | libc::O_NONBLOCK);
            }
        }

        let fd = AsyncFd::new(sock)?;
        Ok(Self { name, fd })
    }
}

fn find_utun_control() -> io::Result<u32> {
    let fd = unsafe { libc::socket(AF_SYSTEM, libc::SOCK_DGRAM, SYSPROTO_CONTROL) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }

    let mut info: libc::ctl_info = unsafe { std::mem::zeroed() };
    let name_bytes = UTUN_CONTROL_NAME.as_bytes();
    let copy_len = name_bytes.len().min(info.ctl_name.len() - 1);
    for (i, &b) in name_bytes.iter().take(copy_len).enumerate() {
        info.ctl_name[i] = b as libc::c_char;
    }

    let ret = unsafe { libc::ioctl(fd, CTLIOCGINFO, &mut info as *mut _) };
    unsafe { libc::close(fd) };
    if ret < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(info.ctl_id)
}

impl AsRawFd for Utun {
    fn as_raw_fd(&self) -> RawFd {
        *self.fd.get_ref()
    }
}

impl Drop for Utun {
    fn drop(&mut self) {
        unsafe { libc::close(*self.fd.get_ref()) };
    }
}

impl TunDevice for Utun {
    fn configure(&self, _addr: IpAddr, _mask: IpAddr, _mtu: u16) -> io::Result<()> {
        Ok(())
    }
    fn name(&self) -> &str {
        &self.name
    }
}

// ── Async I/O ─────────────────────────────────────────────────────────

impl AsyncRead for Utun {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        loop {
            let mut guard = match self.fd.poll_read_ready(cx) {
                Poll::Ready(Ok(g)) => g,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            };
            match guard.try_io(|inner| {
                let ret = unsafe {
                    libc::read(
                        *inner.get_ref(),
                        buf.initialize_unfilled().as_mut_ptr() as *mut libc::c_void,
                        buf.remaining(),
                    )
                };
                if ret < 0 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(ret as usize)
                }
            }) {
                Ok(Ok(n)) => {
                    unsafe { buf.assume_init(n) };
                    buf.advance(n);
                    return Poll::Ready(Ok(()));
                }
                Ok(Err(e)) => return Poll::Ready(Err(e)),
                Err(_) => continue,
            }
        }
    }
}

impl AsyncWrite for Utun {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        loop {
            let mut guard = match self.fd.poll_write_ready(cx) {
                Poll::Ready(Ok(g)) => g,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            };
            match guard.try_io(|inner| {
                let ret = unsafe {
                    libc::write(
                        *inner.get_ref(),
                        buf.as_ptr() as *const libc::c_void,
                        buf.len(),
                    )
                };
                if ret < 0 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(ret as usize)
                }
            }) {
                Ok(Ok(n)) => return Poll::Ready(Ok(n)),
                Ok(Err(e)) => return Poll::Ready(Err(e)),
                Err(_) => continue,
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
