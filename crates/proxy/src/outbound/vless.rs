//! VLESS outbound - TCP connect helpers.
//!
//! TCP outbound connect ([`connect_tcp`]) moved here from `runtime/upstream.rs`
//! so the runtime dispatches via registered TCP outbound capabilities. UDP flow manager
//! glue lives under the VLESS adapter UDP module.

#[cfg(feature = "vless")]
use zero_config::{
    ClientTlsConfig, GrpcConfig, H2Config, HttpUpgradeConfig, QuicConfig, RealityConfig,
    SplitHttpConfig, WebSocketConfig,
};
#[cfg(feature = "vless")]
use zero_core::Session;
#[cfg(feature = "vless")]
use zero_engine::EngineError;
#[cfg(feature = "vless")]
use zero_platform_tokio::TransportConnector;

use crate::runtime::Proxy;
#[cfg(feature = "vless")]
use crate::transport::TcpRelayStream;

/// Establish a VLESS TCP upstream: resolve MUX/QUIC fast paths, dial the
/// server, run the transport + VLESS handshake, return the relay stream.
///
/// Moved from `runtime/upstream.rs`. The runtime dispatches via the adapter
/// trait instead of a per-protocol `connect_via_*` method.
#[cfg(feature = "vless")]
pub(crate) async fn connect_tcp(
    request: VlessTcpConnectRequest<'_>,
) -> Result<TcpRelayStream, EngineError> {
    let VlessTcpConnectRequest {
        proxy,
        session,
        server,
        port,
        uuid,
        flow,
        mux_concurrency,
        mux_idle_timeout_secs,
        tls,
        reality,
        ws,
        grpc,
        h2,
        http_upgrade,
        quic,
        split_http,
    } = request;

    let _ = mux_concurrency;
    let _ = mux_idle_timeout_secs;

    // QUIC uses UDP; handle before TCP connect entirely.
    if let Some(quic) = quic {
        let server_name = quic.server_name.as_deref().unwrap_or(server);
        let quic_stream = crate::transport::connect_quic(server_name, port, quic.insecure).await?;
        return Ok(TcpRelayStream::new(quic_stream));
    }

    let socket = proxy
        .protocols
        .direct_connector()
        .connect_host(server, port, proxy.resolver.as_ref())
        .await?;

    let connector =
        crate::transport::VlessTransportConnector::new(crate::transport::VlessTransportOptions {
            tls,
            reality,
            ws,
            grpc,
            h2,
            http_upgrade,
            split_http,
            source_dir: proxy.config.source_dir(),
        });
    let stream = connector.connect(socket, server, port).await?;

    let is_reality = reality.is_some();
    let mut metered = crate::transport::MeteredStream::new(stream);

    if is_reality {
        use zero_traits::DeferredTcpTunnelProtocol;
        vless::VlessOutbound
            .send_deferred_tcp_tunnel_request(
                &mut metered,
                &vless::VlessFlowTcpTunnelTarget {
                    session,
                    id: &uuid,
                    flow,
                },
            )
            .await?;
        proxy.record_session_outbound_traffic(session.id, metered.drain_traffic());

        Ok(TcpRelayStream::new(
            vless::DeferredVlessResponseStream::new(metered.into_inner()),
        ))
    } else {
        use zero_traits::TcpTunnelProtocol;
        <vless::VlessOutbound as TcpTunnelProtocol<vless::VlessFlowTcpTunnelTarget>>::establish_tcp_tunnel(
            &vless::VlessOutbound,
            &mut metered,
            &vless::VlessFlowTcpTunnelTarget {
                session,
                id: &uuid,
                flow,
            },
        )
        .await?;
        proxy.record_session_outbound_traffic(session.id, metered.drain_traffic());

        Ok(metered.into_inner())
    }
}

#[cfg(feature = "vless")]
pub(crate) struct VlessTcpConnectRequest<'a> {
    pub proxy: &'a Proxy,
    pub session: &'a Session,
    pub server: &'a str,
    pub port: u16,
    pub uuid: [u8; 16],
    pub flow: Option<&'a str>,
    pub mux_concurrency: Option<u32>,
    pub mux_idle_timeout_secs: Option<u64>,
    pub tls: Option<&'a ClientTlsConfig>,
    pub reality: Option<&'a RealityConfig>,
    pub ws: Option<&'a WebSocketConfig>,
    pub grpc: Option<&'a GrpcConfig>,
    pub h2: Option<&'a H2Config>,
    pub http_upgrade: Option<&'a HttpUpgradeConfig>,
    pub quic: Option<&'a QuicConfig>,
    pub split_http: Option<&'a SplitHttpConfig>,
}

/// Apply a VLESS tunnel/session handshake over an existing stream (relay hop).
///
/// Unlike [`connect_tcp`] this does not dial. Flow (Vision) uses the deferred
/// flow target; non-flow uses the plain target.
pub(crate) async fn apply_tcp_hop(
    _proxy: &Proxy,
    mut stream: TcpRelayStream,
    session: &Session,
    uuid: [u8; 16],
    flow: Option<&str>,
) -> Result<TcpRelayStream, EngineError> {
    use zero_traits::TcpTunnelProtocol;
    if flow.is_some() {
        <vless::VlessOutbound as TcpTunnelProtocol<vless::VlessFlowTcpTunnelTarget>>::establish_tcp_tunnel(
            &vless::VlessOutbound,
            &mut stream,
            &vless::VlessFlowTcpTunnelTarget {
                session,
                id: &uuid,
                flow,
            },
        )
        .await
        .map_err(|e| EngineError::Io(std::io::Error::other(e)))?;
    } else {
        <vless::VlessOutbound as TcpTunnelProtocol<vless::VlessTcpTunnelTarget>>::establish_tcp_tunnel(
            &vless::VlessOutbound,
            &mut stream,
            &vless::VlessTcpTunnelTarget {
                session,
                id: &uuid,
            },
        )
        .await
        .map_err(|e| EngineError::Io(std::io::Error::other(e)))?;
    }
    Ok(stream)
}
