use std::path::PathBuf;
use std::sync::Arc;

use zero_config::{
    ClientTlsConfig, GrpcConfig, H2Config, HttpUpgradeConfig, QuicConfig, RealityConfig,
    SplitHttpConfig, WebSocketConfig,
};
use zero_core::Session;
use zero_engine::EngineError;
use zero_platform_tokio::TransportConnector;

use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_flow::managed::{
    managed_stream_connector_flow_from_build, managed_tuple_udp_connection,
    ManagedStreamConnectorFlow, ManagedStreamConnectorFlowBuild, ManagedStreamFlowConnector,
    ManagedTupleUdpSender, SharedManagedUdpConnection,
};
use crate::runtime::Proxy;
use crate::transport::{RelayCarrier, TcpRelayStream};

#[derive(Debug, Clone)]
pub(crate) struct VlessManagedUdpFlowResume {
    mux_pool: vless::mux_pool::MuxConnectionPool,
    protocol: vless::udp::VlessUdpFlowResume,
    transport: VlessManagedUdpTransport,
    mode: VlessManagedUdpMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VlessManagedUdpMode {
    Direct,
    RelayFinalHop,
    RelayPairedTransport,
}

#[derive(Debug, Clone)]
struct VlessManagedUdpTransport {
    tls: Option<ClientTlsConfig>,
    reality: Option<RealityConfig>,
    ws: Option<WebSocketConfig>,
    grpc: Option<GrpcConfig>,
    h2: Option<H2Config>,
    http_upgrade: Option<HttpUpgradeConfig>,
    split_http: Option<SplitHttpConfig>,
    quic: Option<QuicConfig>,
    source_dir: Option<PathBuf>,
}

impl VlessManagedUdpFlowResume {
    pub(super) fn direct(
        mux_pool: vless::mux_pool::MuxConnectionPool,
        protocol: vless::udp::VlessUdpFlowResume,
        transport: crate::transport::VlessUdpTransportOptions<'_>,
    ) -> Self {
        Self {
            mux_pool,
            protocol,
            transport: VlessManagedUdpTransport::from_options(transport),
            mode: VlessManagedUdpMode::Direct,
        }
    }

    pub(super) fn relay_final_hop(
        mux_pool: vless::mux_pool::MuxConnectionPool,
        protocol: vless::udp::VlessUdpFlowResume,
        transport: crate::transport::VlessUdpTransportOptions<'_>,
    ) -> Self {
        Self {
            mux_pool,
            protocol,
            transport: VlessManagedUdpTransport::from_options(transport),
            mode: VlessManagedUdpMode::RelayFinalHop,
        }
    }

    pub(super) fn relay_paired_transport(
        mux_pool: vless::mux_pool::MuxConnectionPool,
        protocol: vless::udp::VlessUdpFlowResume,
        transport: crate::transport::VlessUdpTransportOptions<'_>,
    ) -> Self {
        Self {
            mux_pool,
            protocol,
            transport: VlessManagedUdpTransport::from_options(transport),
            mode: VlessManagedUdpMode::RelayPairedTransport,
        }
    }

    async fn establish_direct_connection(
        &self,
        proxy: &Proxy,
        session: &Session,
        endpoint: OutboundEndpoint<'_>,
    ) -> Result<SharedManagedUdpConnection, EngineError> {
        if let Some(connection) = self.try_establish_mux_connection(proxy, endpoint).await? {
            return Ok(managed_tuple_udp_connection(Arc::new(connection)));
        }

        let socket = proxy
            .protocols
            .direct_connector()
            .connect_host(endpoint.server, endpoint.port, proxy.resolver.as_ref())
            .await?;

        let connector = crate::transport::VlessUdpTransportConnector::new(self.transport.options());
        let stream: TcpRelayStream = connector
            .connect(socket, endpoint.server, endpoint.port)
            .await?;
        let connection =
            vless::udp::establish_udp_flow_with_resume(stream, session, &self.protocol)
                .await
                .map_err(engine_error)?;
        Ok(managed_tuple_udp_connection(Arc::new(connection)))
    }

    async fn try_establish_mux_connection(
        &self,
        proxy: &Proxy,
        endpoint: OutboundEndpoint<'_>,
    ) -> Result<Option<vless::udp::VlessUdpFlowConnection>, EngineError> {
        if !self.protocol.mux_flow_enabled() {
            return Ok(None);
        }

        let Ok((_session_id, up_tx, down_rx)) = crate::adapters::vless::mux_pool::open_udp_stream(
            &self.mux_pool,
            proxy,
            endpoint.server,
            endpoint.port,
            self.protocol.mux_pool_identity(),
            self.transport.tls.as_ref(),
            self.transport.reality.as_ref(),
            8,
        )
        .await
        else {
            return Ok(None);
        };

        Ok(Some(vless::udp::start_mux_udp_flow(up_tx, down_rx)))
    }

    async fn establish_relay_connection(
        &self,
        stream: TcpRelayStream,
        session: &Session,
        endpoint: OutboundEndpoint<'_>,
    ) -> Result<SharedManagedUdpConnection, EngineError> {
        let stream = match self.mode {
            VlessManagedUdpMode::Direct => {
                return Err(EngineError::Io(std::io::Error::other(
                    "expected direct VLESS UDP flow to establish without relay carrier",
                )));
            }
            VlessManagedUdpMode::RelayFinalHop => {
                crate::transport::build_vless_outbound_transport_over_stream(
                    crate::transport::VlessFinalHopTransportRequest {
                        carrier: RelayCarrier {
                            stream,
                            server: endpoint.server.to_string(),
                            port: endpoint.port,
                        },
                        options: self.transport.options().stream_options(),
                    },
                )
                .await?
            }
            VlessManagedUdpMode::RelayPairedTransport => stream,
        };

        let connection =
            vless::udp::establish_udp_flow_with_resume(stream, session, &self.protocol)
                .await
                .map_err(engine_error)?;
        Ok(managed_tuple_udp_connection(Arc::new(connection)))
    }
}

impl VlessManagedUdpTransport {
    fn from_options(options: crate::transport::VlessUdpTransportOptions<'_>) -> Self {
        Self {
            tls: options.tls.cloned(),
            reality: options.reality.cloned(),
            ws: options.ws.cloned(),
            grpc: options.grpc.cloned(),
            h2: options.h2.cloned(),
            http_upgrade: options.http_upgrade.cloned(),
            split_http: options.split_http.cloned(),
            quic: options.quic.cloned(),
            source_dir: options.source_dir.map(PathBuf::from),
        }
    }

    fn options(&self) -> crate::transport::VlessUdpTransportOptions<'_> {
        crate::transport::VlessUdpTransportOptions {
            tls: self.tls.as_ref(),
            reality: self.reality.as_ref(),
            ws: self.ws.as_ref(),
            grpc: self.grpc.as_ref(),
            h2: self.h2.as_ref(),
            http_upgrade: self.http_upgrade.as_ref(),
            split_http: self.split_http.as_ref(),
            quic: self.quic.as_ref(),
            source_dir: self.source_dir.as_deref(),
        }
    }
}

impl ManagedStreamConnectorFlowBuild for vless::udp::VlessUdpConnectorFlow {
    fn into_parts(self) -> (String, bool) {
        vless::udp::VlessUdpConnectorFlow::into_parts(self)
    }
}

#[async_trait::async_trait]
impl ManagedStreamFlowConnector for VlessManagedUdpFlowResume {
    fn connector_flow(
        &self,
        endpoint: OutboundEndpoint<'_>,
        session_id: u64,
    ) -> ManagedStreamConnectorFlow {
        managed_stream_connector_flow_from_build(vless::udp::connector_flow_from_resume(
            &self.protocol,
            endpoint.server,
            endpoint.port,
            session_id,
        ))
    }

    async fn establish_direct(
        &self,
        proxy: &Proxy,
        session: &Session,
        endpoint: OutboundEndpoint<'_>,
    ) -> Result<SharedManagedUdpConnection, EngineError> {
        self.establish_direct_connection(proxy, session, endpoint)
            .await
    }

    async fn establish_relay(
        &self,
        stream: TcpRelayStream,
        _tls_server_name: Option<&str>,
        _proxy: Option<&Proxy>,
        session: &Session,
        endpoint: OutboundEndpoint<'_>,
    ) -> Result<SharedManagedUdpConnection, EngineError> {
        self.establish_relay_connection(stream, session, endpoint)
            .await
    }
}

#[async_trait::async_trait]
impl ManagedTupleUdpSender for vless::udp::VlessUdpFlowConnection {
    async fn send(
        &self,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        vless::udp::VlessUdpFlowConnection::send(self, target, port, payload)
            .await
            .map_err(engine_error)
    }

    fn subscribe_responses(&self) -> vless::udp::VlessUdpFlowResponseReceiver {
        vless::udp::VlessUdpFlowConnection::subscribe_responses(self)
    }

    fn closed_message(&self) -> &'static str {
        "vless upstream closed"
    }
}

fn engine_error(error: zero_core::Error) -> EngineError {
    EngineError::Io(std::io::Error::other(error.to_string()))
}
