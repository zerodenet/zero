use std::future::Future;

use zero_core::Error;

use crate::runtime::udp_flow::helpers::{
    UdpChainResponseParts, UdpDirectResponseParts, UdpUpstreamResponseParts,
};

pub(crate) async fn write_direct_response<F, Fut>(
    response: &UdpDirectResponseParts<'_, '_>,
    write: F,
) -> Result<usize, Error>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<usize, Error>>,
{
    let written = write().await?;
    response.accounting.record_sent(written);
    Ok(written)
}

pub(crate) fn write_direct_response_sync<F>(
    response: &UdpDirectResponseParts<'_, '_>,
    write: F,
) -> Result<usize, Error>
where
    F: FnOnce() -> Result<usize, Error>,
{
    let written = write()?;
    response.accounting.record_sent(written);
    Ok(written)
}

pub(crate) async fn write_upstream_response<F, Fut>(
    response: &UdpUpstreamResponseParts<'_>,
    write: F,
) -> Result<usize, Error>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<usize, Error>>,
{
    let written = write().await?;
    response.accounting.record_sent(written);
    Ok(written)
}

pub(crate) fn write_upstream_response_sync<F>(
    response: &UdpUpstreamResponseParts<'_>,
    write: F,
) -> Result<usize, Error>
where
    F: FnOnce() -> Result<usize, Error>,
{
    let written = write()?;
    response.accounting.record_sent(written);
    Ok(written)
}

pub(crate) async fn write_chain_response<F, Fut>(
    response: &UdpChainResponseParts<'_>,
    write: F,
) -> Result<usize, Error>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<usize, Error>>,
{
    let written = write().await?;
    response.accounting.record_sent(written);
    Ok(written)
}

pub(crate) fn write_chain_response_sync<F>(
    response: &UdpChainResponseParts<'_>,
    write: F,
) -> Result<usize, Error>
where
    F: FnOnce() -> Result<usize, Error>,
{
    let written = write()?;
    response.accounting.record_sent(written);
    Ok(written)
}
