//! Managed stream relay handler facade.
//!
//! The root stays as a facade so packet-flow and relay-flow handler adapters do
//! not regrow into one mixed implementation bucket.

#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
mod packet;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
mod relay;
