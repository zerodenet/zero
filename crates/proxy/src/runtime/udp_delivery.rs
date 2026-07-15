//! UDP response writer execution with neutral traffic accounting.

use std::future::Future;

mod helpers;

pub(crate) use helpers::*;

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
pub(crate) async fn write_direct_response<F, Fut, E>(
    response: &UdpDirectResponseParts<'_>,
    write: F,
) -> Result<usize, E>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<usize, E>>,
{
    let written = write().await?;
    response.accounting.record_sent(written);
    Ok(written)
}

#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
pub(crate) async fn write_optional_direct_response<F, Fut, E>(
    response: &UdpDirectResponseParts<'_>,
    write: F,
) -> Result<Option<usize>, E>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<Option<usize>, E>>,
{
    let written = write().await?;
    if let Some(written) = written {
        response.accounting.record_sent(written);
    }
    Ok(written)
}

#[cfg(feature = "socks5")]
pub(crate) async fn write_upstream_response<F, Fut, E>(
    response: &UdpUpstreamResponseParts,
    write: F,
) -> Result<usize, E>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<usize, E>>,
{
    let written = write().await?;
    response.accounting.record_sent(written);
    Ok(written)
}

#[cfg(all(
    feature = "socks5",
    any(feature = "hysteria2", feature = "shadowsocks")
))]
pub(crate) async fn write_optional_upstream_response<F, Fut, E>(
    response: &UdpUpstreamResponseParts,
    write: F,
) -> Result<Option<usize>, E>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<Option<usize>, E>>,
{
    let written = write().await?;
    if let Some(written) = written {
        response.accounting.record_sent(written);
    }
    Ok(written)
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
pub(crate) async fn write_chain_response<F, Fut, E>(
    response: &UdpChainResponseParts,
    write: F,
) -> Result<usize, E>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<usize, E>>,
{
    let written = write().await?;
    response.accounting.record_sent(written);
    Ok(written)
}

#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
pub(crate) async fn write_optional_chain_response<F, Fut, E>(
    response: &UdpChainResponseParts,
    write: F,
) -> Result<Option<usize>, E>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<Option<usize>, E>>,
{
    let written = write().await?;
    if let Some(written) = written {
        response.accounting.record_sent(written);
    }
    Ok(written)
}
