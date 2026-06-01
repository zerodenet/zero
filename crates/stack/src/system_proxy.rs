//! OS-level system proxy configuration.
//!
//! On Windows: uses WinINET API (`WinHttpSetDefaultProxyConfiguration`) to
//! set the system-wide HTTP proxy.  On drop, restores the original proxy.
//! On Linux/macOS: the user must configure iptables/pf manually (for now).
//!
//! The proxy setting affects applications that respect system proxy
//! (browsers, curl, most CLI tools).  For full traffic capture (all TCP),
//! use TUN mode.

#[cfg(target_os = "windows")]
mod windows_proxy {
    use std::{io, ptr};

    use windows::core::PWSTR;
    use windows::Win32::Networking::WinHttp::{
        WinHttpGetDefaultProxyConfiguration, WinHttpSetDefaultProxyConfiguration,
        WINHTTP_ACCESS_TYPE_DEFAULT_PROXY, WINHTTP_ACCESS_TYPE_NAMED_PROXY,
        WINHTTP_ACCESS_TYPE_NO_PROXY, WINHTTP_PROXY_INFO,
    };

    /// Saves and restores the Windows system proxy on drop.
    pub struct ProxyGuard {
        original: WINHTTP_PROXY_INFO,
        /// Whether original had NO_PROXY (i.e. was not set before).
        was_no_proxy: bool,
    }

    impl ProxyGuard {
        /// Set the system proxy to `127.0.0.1:{port}`.
        ///
        /// Returns a guard that restores the previous proxy configuration
        /// when dropped.  Returns `None` if the proxy was already set.
        pub fn set(port: u16) -> io::Result<Self> {
            // Read current proxy settings.
            let mut original = WINHTTP_PROXY_INFO {
                dwAccessType: WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                lpszProxy: PWSTR(ptr::null_mut()),
                lpszProxyBypass: PWSTR(ptr::null_mut()),
            };

            let was_no_proxy = unsafe {
                WinHttpGetDefaultProxyConfiguration(&mut original).is_err()
            };

            // Build the proxy string: "127.0.0.1:{port}".
            let proxy_str = format!("127.0.0.1:{port}");
            let proxy_wide: Vec<u16> = proxy_str
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            let bypass_wide: Vec<u16> = vec![0]; // empty bypass list

            let new_proxy = WINHTTP_PROXY_INFO {
                dwAccessType: WINHTTP_ACCESS_TYPE_NAMED_PROXY,
                lpszProxy: PWSTR(proxy_wide.as_ptr() as *mut _),
                lpszProxyBypass: PWSTR(bypass_wide.as_ptr() as *mut _),
            };

            unsafe {
                WinHttpSetDefaultProxyConfiguration(&new_proxy as *const _ as *mut _)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("{e}")))?;
            }

            Ok(Self { original, was_no_proxy })
        }
    }

    impl Drop for ProxyGuard {
        fn drop(&mut self) {
            if self.was_no_proxy {
                // Restore "no proxy".
                let no_proxy = WINHTTP_PROXY_INFO {
                    dwAccessType: WINHTTP_ACCESS_TYPE_NO_PROXY,
                    lpszProxy: PWSTR(ptr::null_mut()),
                    lpszProxyBypass: PWSTR(ptr::null_mut()),
                };
                unsafe {
                    WinHttpSetDefaultProxyConfiguration(&no_proxy as *const _ as *mut _).ok();
                }
            } else {
                unsafe {
                    WinHttpSetDefaultProxyConfiguration(&self.original as *const _ as *mut _).ok();
                }
            }
        }
    }
}

#[cfg(target_os = "windows")]
pub use windows_proxy::ProxyGuard;

// ── Stub for non-Windows platforms ────────────────────────────────────

#[cfg(not(target_os = "windows"))]
pub struct ProxyGuard;

#[cfg(not(target_os = "windows"))]
impl ProxyGuard {
    #[allow(unused_variables)]
    pub fn set(port: u16) -> std::io::Result<Self> {
        Ok(Self)
    }
}
