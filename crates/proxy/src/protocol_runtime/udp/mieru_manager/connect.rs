use super::super::MieruUdpPeer;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use mieru::MieruUdpFlowIo;
use zero_engine::EngineError;

pub(super) struct EstablishedSession {
    pub(super) stream: TcpRelayStream,
    pub(super) flow_io: MieruUdpFlowIo,
}

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

pub(super) async fn open_udp_flow(
    mut stream: TcpRelayStream,
    username: &str,
    password: &str,
) -> Result<EstablishedSession, EngineError> {
    let flow_io = MieruUdpFlowIo::establish(&mut stream, username, password)
        .await
        .map_err(|error| {
            EngineError::Io(std::io::Error::other(format!(
                "mieru udp associate: {error}"
            )))
        })?;

    Ok(EstablishedSession { stream, flow_io })
}
