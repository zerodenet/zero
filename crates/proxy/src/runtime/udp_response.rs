use std::future::Future;

use crate::runtime::udp_flow::helpers::{
    UdpChainResponseParts, UdpDirectResponseParts, UdpUpstreamResponseParts,
};

pub(crate) async fn write_direct_response<F, Fut, E>(
    response: &UdpDirectResponseParts<'_, '_>,
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

pub(crate) async fn write_optional_direct_response<F, Fut, E>(
    response: &UdpDirectResponseParts<'_, '_>,
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

pub(crate) async fn write_upstream_response<F, Fut, E>(
    response: &UdpUpstreamResponseParts<'_>,
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

pub(crate) async fn write_optional_upstream_response<F, Fut, E>(
    response: &UdpUpstreamResponseParts<'_>,
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

pub(crate) async fn write_chain_response<F, Fut, E>(
    response: &UdpChainResponseParts<'_>,
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

pub(crate) async fn write_optional_chain_response<F, Fut, E>(
    response: &UdpChainResponseParts<'_>,
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
