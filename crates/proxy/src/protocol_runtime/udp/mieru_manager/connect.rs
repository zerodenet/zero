use super::super::MieruUdpPeer;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use zero_engine::EngineError;

pub(super) async fn direct_stream(
    proxy: &Proxy,
    peer: &MieruUdpPeer<'_>,
) -> Result<TcpRelayStream, EngineError> {
    let socket = proxy
        .protocols
        .direct_connector()
        .connect_host(
            peer.endpoint.server,
            peer.endpoint.port,
            proxy.resolver.as_ref(),
        )
        .await?;
    Ok(TcpRelayStream::new(socket))
}
