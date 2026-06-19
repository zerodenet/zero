//! VLESS outbound — TCP connect and UDP types.
//!
//! TCP outbound connect ([`connect_tcp`]) moved here from `runtime/upstream.rs`
//! so the runtime dispatches via the `ProtocolAdapter` trait. The VLESS UDP
//! types and manager live in `crate::runtime::vless_udp` so that inbound
//! handlers can import them without depending on this module.

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

#[cfg(feature = "vless")]
use crate::runtime::Proxy;
#[cfg(feature = "vless")]
use crate::transport::TcpRelayStream;

/// Establish a VLESS TCP upstream: resolve MUX/QUIC fast paths, dial the
/// server, run the transport + VLESS handshake, return the relay stream.
///
/// Moved from `runtime/upstream.rs`. The runtime dispatches via the adapter
/// trait instead of a per-protocol `connect_via_*` method.
#[cfg(feature = "vless")]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn connect_tcp(
    proxy: &Proxy,
    session: &Session,
    server: &str,
    port: u16,
    id: &str,
    flow: Option<&str>,
    mux_concurrency: Option<u32>,
    mux_idle_timeout_secs: Option<u64>,
    tls: Option<&ClientTlsConfig>,
    reality: Option<&RealityConfig>,
    ws: Option<&WebSocketConfig>,
    grpc: Option<&GrpcConfig>,
    h2: Option<&H2Config>,
    http_upgrade: Option<&HttpUpgradeConfig>,
    quic: Option<&QuicConfig>,
    split_http: Option<&SplitHttpConfig>,
) -> Result<TcpRelayStream, EngineError> {
    let id = vless::parse_uuid(id)?;

    // If MUX flow is configured, use connection pool.
    if flow == Some("xtls-rprx-vision") {
        return proxy
            .mux_pool
            .open_stream(
                proxy,
                session,
                server.to_owned(),
                port,
                &id,
                tls,
                reality,
                mux_concurrency.unwrap_or(8),
                mux_idle_timeout_secs.unwrap_or(300),
            )
            .await;
    }

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

    let connector = crate::transport::VlessTransportConnector::new(
        tls,
        reality,
        ws,
        grpc,
        h2,
        http_upgrade,
        split_http,
        proxy.config.source_dir(),
    );
    let stream = connector.connect(socket, server, port).await?;

    let is_reality = reality.is_some();
    let mut metered = crate::transport::MeteredStream::new(stream);

    if is_reality {
        use zero_traits::DeferredTcpTunnelProtocol;
        proxy
            .protocols
            .vless_outbound_protocol()
            .send_deferred_tcp_tunnel_request(
                &mut metered,
                &vless::VlessFlowTcpTunnelTarget {
                    session,
                    id: &id,
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
            &proxy.protocols.vless_outbound_protocol(),
            &mut metered,
            &vless::VlessFlowTcpTunnelTarget {
                session,
                id: &id,
                flow,
            },
        )
        .await?;
        proxy.record_session_outbound_traffic(session.id, metered.drain_traffic());

        Ok(metered.into_inner())
    }
}

/// Apply a VLESS tunnel/session handshake over an existing stream (relay hop).
///
/// Unlike [`connect_tcp`] this does not dial. Flow (Vision) uses the deferred
/// flow target; non-flow uses the plain target.
pub(crate) async fn apply_tcp_hop(
    proxy: &Proxy,
    mut stream: TcpRelayStream,
    session: &Session,
    id: &str,
    flow: Option<&str>,
) -> Result<TcpRelayStream, EngineError> {
    let uuid = vless::parse_uuid(id)?;
    use zero_traits::TcpTunnelProtocol;
    if flow.is_some() {
        <vless::VlessOutbound as TcpTunnelProtocol<vless::VlessFlowTcpTunnelTarget>>::establish_tcp_tunnel(
            &proxy.protocols.vless_outbound_protocol(),
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
            &proxy.protocols.vless_outbound_protocol(),
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
