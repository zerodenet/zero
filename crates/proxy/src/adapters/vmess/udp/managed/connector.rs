use std::path::PathBuf;
use std::sync::Arc;

use zero_config::{ClientTlsConfig, GrpcConfig, WebSocketConfig};
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
pub(crate) struct VmessManagedUdpFlowResume {
    mux_pool: vmess::mux::VmessMuxConnectionPool,
    protocol: vmess::udp::VmessUdpFlowResume,
    transport: VmessManagedUdpTransport,
    mux_concurrency: Option<u32>,
}

#[derive(Debug, Clone)]
struct VmessManagedUdpTransport {
    tls: Option<ClientTlsConfig>,
    ws: Option<WebSocketConfig>,
    grpc: Option<GrpcConfig>,
    source_dir: Option<PathBuf>,
}

impl VmessManagedUdpFlowResume {
    pub(super) fn new(
        mux_pool: vmess::mux::VmessMuxConnectionPool,
        protocol: vmess::udp::VmessUdpFlowResume,
        mux_concurrency: Option<u32>,
        transport: crate::transport::VmessTransportOptions<'_>,
    ) -> Self {
        Self {
            mux_pool,
            protocol,
            transport: VmessManagedUdpTransport::from_options(transport),
            mux_concurrency,
        }
    }

    async fn establish_direct_connection(
        &self,
        proxy: &Proxy,
        session: &Session,
        endpoint: OutboundEndpoint<'_>,
    ) -> Result<SharedManagedUdpConnection, EngineError> {
        if let Some(max_concurrency) = self.mux_concurrency {
            let mux_stream = crate::adapters::vmess::mux_pool::open_udp_stream(
                &self.mux_pool,
                proxy,
                session,
                endpoint.server,
                endpoint.port,
                self.protocol.mux_pool_identity(),
                self.transport.tls.as_ref(),
                self.transport.ws.as_ref(),
                self.transport.grpc.as_ref(),
                max_concurrency,
            )
            .await?;
            return Ok(managed_tuple_udp_connection(Arc::new(
                vmess::udp::start_udp_flow(mux_stream),
            )));
        }

        let socket = proxy
            .protocols
            .direct_connector()
            .connect_host(endpoint.server, endpoint.port, proxy.resolver.as_ref())
            .await?;
        let connector = crate::transport::VmessTransportConnector::new(self.transport.options());
        let stream = connector
            .connect(socket, endpoint.server, endpoint.port)
            .await?;
        let connection =
            vmess::udp::establish_udp_flow_with_resume(stream, session, &self.protocol)
                .await
                .map_err(engine_error)?;
        Ok(managed_tuple_udp_connection(Arc::new(connection)))
    }

    async fn establish_relay_connection(
        &self,
        stream: TcpRelayStream,
        session: &Session,
        endpoint: OutboundEndpoint<'_>,
    ) -> Result<SharedManagedUdpConnection, EngineError> {
        let stream = crate::transport::build_vmess_outbound_transport_over_stream(
            crate::transport::VmessFinalHopTransportRequest {
                carrier: RelayCarrier {
                    stream,
                    server: endpoint.server.to_string(),
                    port: endpoint.port,
                },
                options: self.transport.options(),
            },
        )
        .await?;
        let connection =
            vmess::udp::establish_udp_flow_with_resume(stream, session, &self.protocol)
                .await
                .map_err(engine_error)?;
        Ok(managed_tuple_udp_connection(Arc::new(connection)))
    }
}

impl VmessManagedUdpTransport {
    fn from_options(options: crate::transport::VmessTransportOptions<'_>) -> Self {
        Self {
            tls: options.tls.cloned(),
            ws: options.ws.cloned(),
            grpc: options.grpc.cloned(),
            source_dir: options.source_dir.map(PathBuf::from),
        }
    }

    fn options(&self) -> crate::transport::VmessTransportOptions<'_> {
        crate::transport::VmessTransportOptions {
            tls: self.tls.as_ref(),
            ws: self.ws.as_ref(),
            grpc: self.grpc.as_ref(),
            source_dir: self.source_dir.as_deref(),
        }
    }
}

impl ManagedStreamConnectorFlowBuild for vmess::udp::VmessUdpConnectorFlow {
    fn into_parts(self) -> (String, bool) {
        vmess::udp::VmessUdpConnectorFlow::into_parts(self)
    }
}

#[async_trait::async_trait]
impl ManagedStreamFlowConnector for VmessManagedUdpFlowResume {
    fn connector_flow(
        &self,
        endpoint: OutboundEndpoint<'_>,
        session_id: u64,
    ) -> ManagedStreamConnectorFlow {
        managed_stream_connector_flow_from_build(vmess::udp::connector_flow_from_resume(
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
impl ManagedTupleUdpSender for vmess::udp::VmessUdpFlowConnection {
    async fn send(
        &self,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        vmess::udp::VmessUdpFlowConnection::send(self, target, port, payload)
            .await
            .map_err(engine_error)
    }

    fn subscribe_responses(&self) -> vmess::udp::VmessUdpFlowResponseReceiver {
        vmess::udp::VmessUdpFlowConnection::subscribe_responses(self)
    }

    fn closed_message(&self) -> &'static str {
        "vmess upstream closed"
    }
}

fn engine_error(error: zero_core::Error) -> EngineError {
    EngineError::Io(std::io::Error::other(error.to_string()))
}
