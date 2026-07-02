use std::sync::Arc;

use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_flow::managed::{
    managed_packet_udp_connection, managed_stream_connector_flow_from_build,
    ManagedPacketUdpSender, ManagedStreamConnectorFlow, ManagedStreamFlowConnector,
    SharedManagedUdpConnection,
};
use crate::runtime::Proxy;
use crate::transport::{
    open_trojan_udp_tls_relay_stream, open_trojan_udp_tls_stream, TcpRelayStream, TrojanTlsProfile,
    TrojanUdpTlsOptions,
};
use zero_core::Session;
use zero_engine::EngineError;

pub(super) struct TrojanManagedStreamConnector;

impl crate::runtime::udp_flow::managed::ManagedStreamConnectorFlowBuild
    for trojan::udp::TrojanUdpConnectorFlow
{
    fn into_parts(self) -> (String, bool) {
        trojan::udp::TrojanUdpConnectorFlow::into_parts(self)
    }
}

#[async_trait::async_trait]
impl ManagedStreamFlowConnector<trojan::udp::TrojanUdpFlowResume> for TrojanManagedStreamConnector {
    fn connector_flow(
        &self,
        resume: &trojan::udp::TrojanUdpFlowResume,
        endpoint: OutboundEndpoint<'_>,
        session_id: u64,
    ) -> ManagedStreamConnectorFlow {
        let flow = trojan::udp::connector_flow_from_resume(
            resume,
            endpoint.server,
            endpoint.port,
            session_id,
        );
        managed_stream_connector_flow_from_build(flow)
    }

    async fn establish_direct(
        &self,
        proxy: &Proxy,
        session: &Session,
        endpoint: OutboundEndpoint<'_>,
        resume: trojan::udp::TrojanUdpFlowResume,
    ) -> Result<SharedManagedUdpConnection, EngineError> {
        let tls_stream = open_udp_tls_stream(proxy, endpoint, &resume).await?;
        packet_stream(session, tls_stream, resume).await
    }

    async fn establish_relay(
        &self,
        stream: TcpRelayStream,
        tls_server_name: Option<&str>,
        proxy: Option<&Proxy>,
        session: &Session,
        endpoint: OutboundEndpoint<'_>,
        resume: trojan::udp::TrojanUdpFlowResume,
    ) -> Result<SharedManagedUdpConnection, EngineError> {
        let proxy = proxy.ok_or_else(|| {
            EngineError::Io(std::io::Error::other(
                "expected proxy context for Trojan UDP relay flow",
            ))
        })?;
        let tls_stream =
            open_udp_tls_relay_stream(stream, tls_server_name, proxy, endpoint, &resume).await?;
        packet_stream(session, tls_stream, resume).await
    }
}

async fn open_udp_tls_stream(
    proxy: &Proxy,
    endpoint: OutboundEndpoint<'_>,
    resume: &trojan::udp::TrojanUdpFlowResume,
) -> Result<TcpRelayStream, EngineError> {
    let upstream = proxy
        .protocols
        .direct_connector()
        .connect_host(endpoint.server, endpoint.port, proxy.resolver.as_ref())
        .await?;

    open_trojan_udp_tls_stream(
        upstream,
        udp_tls_options(
            proxy,
            endpoint,
            trojan::udp::connector_tls_profile_from_resume(resume, None),
        ),
    )
    .await
}

async fn open_udp_tls_relay_stream(
    stream: TcpRelayStream,
    tls_server_name: Option<&str>,
    proxy: &Proxy,
    endpoint: OutboundEndpoint<'_>,
    resume: &trojan::udp::TrojanUdpFlowResume,
) -> Result<TcpRelayStream, EngineError> {
    open_trojan_udp_tls_relay_stream(
        stream,
        udp_tls_options(
            proxy,
            endpoint,
            trojan::udp::connector_tls_profile_from_resume(resume, tls_server_name),
        ),
    )
    .await
}

fn udp_tls_options<'a>(
    proxy: &'a Proxy,
    endpoint: OutboundEndpoint<'a>,
    tls_profile: trojan::udp::TrojanUdpTlsProfile,
) -> TrojanUdpTlsOptions<'a> {
    let (server_name, insecure, client_fingerprint) = tls_profile.into_parts();
    TrojanUdpTlsOptions {
        tls_profile: TrojanTlsProfile::from_parts(
            server_name.as_deref(),
            insecure,
            client_fingerprint.as_deref(),
        ),
        source_dir: proxy.config.source_dir(),
        server: endpoint.server,
    }
}

async fn packet_stream(
    session: &Session,
    stream: TcpRelayStream,
    resume: trojan::udp::TrojanUdpFlowResume,
) -> Result<SharedManagedUdpConnection, EngineError> {
    let connection = trojan::udp::establish_udp_flow_with_resume(stream, session, &resume)
        .await
        .map_err(|error| EngineError::Io(std::io::Error::other(format!("{error}"))))?;
    Ok(managed_packet_udp_connection(Arc::new(connection)))
}

#[async_trait::async_trait]
impl ManagedPacketUdpSender for trojan::udp::TrojanUdpFlowConnection {
    async fn send(
        &self,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        trojan::udp::TrojanUdpFlowConnection::send(self, target, port, payload)
            .await
            .map_err(|error| EngineError::Io(std::io::Error::other(format!("{error}"))))
    }

    fn subscribe_responses(&self) -> trojan::udp::TrojanUdpFlowResponseReceiver {
        trojan::udp::TrojanUdpFlowConnection::subscribe_responses(self)
    }

    fn closed_message(&self) -> &'static str {
        "trojan upstream closed"
    }
}
