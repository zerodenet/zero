use zero_core::Session;
use zero_engine::EngineError;
use zero_platform_tokio::TransportConnector;

use super::model::VlessUdpUpstream;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

fn upstream_from_stream(session_id: u64, flow: vless::VlessUdpFlowHandle) -> VlessUdpUpstream {
    VlessUdpUpstream {
        session_id,
        connection: vless::VlessUdpFlowConnection::new(flow),
    }
}

pub(super) async fn over_stream(
    proxy: &Proxy,
    session: &Session,
    config: vless::VlessUdpFlowConfig<'_>,
    initial_payload: &[u8],
    stream: TcpRelayStream,
) -> Result<VlessUdpUpstream, EngineError> {
    let established = config
        .establish_flow_with_initial_packet(stream, session, initial_payload)
        .await?;
    proxy.record_session_outbound_tx(session.id, established.initial_packet_len as u64);
    Ok(upstream_from_stream(session.id, established.handle))
}

pub(super) async fn direct(
    proxy: &Proxy,
    session: &Session,
    server: &str,
    port: u16,
    config: vless::VlessUdpFlowConfig<'_>,
    initial_payload: &[u8],
    transport: Option<&crate::transport::VlessUdpTransportOptions<'_>>,
) -> Result<VlessUdpUpstream, EngineError> {
    let socket = proxy
        .protocols
        .direct_connector()
        .connect_host(server, port, proxy.resolver.as_ref())
        .await?;

    let stream: TcpRelayStream = match transport {
        Some(t) => {
            let connector = crate::transport::VlessUdpTransportConnector::new(*t);
            connector.connect(socket, server, port).await?
        }
        None => socket.into(),
    };

    over_stream(proxy, session, config, initial_payload, stream).await
}
