//! Kernel pipe abstraction.
//!
//! The proxy runtime is an orchestration engine. This trait is the top-level
//! runtime boundary: TCP and UDP are the two core pipe implementations, while
//! concrete protocols plug into those pipes through protocol traits and
//! dispatch categories.

mod contract;
mod tcp;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
mod udp;

pub(crate) use contract::KernelPipe;
pub(crate) use tcp::{TcpPipe, TcpPipeInput};
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) use udp::{UdpPipe, UdpPipeInput};
