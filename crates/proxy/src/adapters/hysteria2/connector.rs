use zero_core::Session;
use zero_engine::EngineError;

use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_flow::managed::ManagedDatagramConnectorFlowBuild;
use crate::runtime::udp_flow::packet_path::{DatagramCodec, PacketPathCarrierDescriptorBuild};
use crate::transport::{
    Hysteria2QuicProfile, Hysteria2Stream, QuicConnectionOptions, TcpRelayStream,
};

pub(super) struct Hysteria2Connector {
    server: String,
    port: u16,
    quic_profile: Hysteria2QuicProfile,
}

impl Hysteria2Connector {
    pub(super) fn new(server: &str, port: u16, client_fingerprint: Option<&str>) -> Self {
        Self {
            server: server.to_owned(),
            port,
            quic_profile: Hysteria2QuicProfile::from_parts(client_fingerprint),
        }
    }

    fn from_udp_profile(
        server: &str,
        port: u16,
        profile: hysteria2::udp::Hysteria2UdpConnectorProfile,
    ) -> Self {
        Self {
            server: server.to_owned(),
            port,
            quic_profile: Hysteria2QuicProfile::from_parts(profile.client_fingerprint()),
        }
    }

    async fn connect_raw(
        &self,
        profile: &hysteria2::Hysteria2OutboundProfile,
    ) -> Result<quinn::Connection, EngineError> {
        let conn = self.open_quic_connection().await?;

        let (send, recv) = conn.open_bi().await.map_err(|error| {
            EngineError::Io(std::io::Error::other(format!("hysteria2 open_bi: {error}")))
        })?;
        let mut stream = Hysteria2Stream::new(send, recv);
        profile
            .authenticate_connection(&conn, &mut stream)
            .await
            .map_err(EngineError::Core)?;

        Ok(conn)
    }

    async fn open_quic_connection(&self) -> Result<quinn::Connection, EngineError> {
        crate::transport::open_hysteria2_quic_connection(QuicConnectionOptions {
            server: &self.server,
            port: self.port,
            alpn: vec![b"hysteria2".to_vec()],
            quic_profile: self.quic_profile.clone(),
            datagram_receive_buffer_size: Some(65536),
        })
        .await
    }

    async fn connect_raw_with_udp_profile(
        &self,
        profile: &hysteria2::udp::Hysteria2UdpConnectorProfile,
    ) -> Result<quinn::Connection, EngineError> {
        let conn = self.open_quic_connection().await?;

        let (send, recv) = conn.open_bi().await.map_err(|error| {
            EngineError::Io(std::io::Error::other(format!("hysteria2 open_bi: {error}")))
        })?;
        let mut stream = Hysteria2Stream::new(send, recv);
        profile
            .authenticate_connection(&conn, &mut stream)
            .await
            .map_err(EngineError::Core)?;

        Ok(conn)
    }

    pub(super) async fn connect(
        &self,
        session: &Session,
        profile: &hysteria2::Hysteria2OutboundProfile,
    ) -> Result<Hysteria2Stream, EngineError> {
        let conn = self.connect_raw(profile).await?;
        let (send, recv) = conn.open_bi().await.map_err(|error| {
            EngineError::Io(std::io::Error::other(format!("hysteria2 open_bi: {error}")))
        })?;

        let mut stream = Hysteria2Stream::new(send, recv);
        hysteria2::Hysteria2Outbound
            .establish_tcp_connect(&mut stream, session)
            .await
            .map_err(EngineError::Core)?;

        Ok(stream)
    }
}

async fn open_udp_profile_connection(
    server: &str,
    port: u16,
    connector_profile: hysteria2::udp::Hysteria2UdpConnectorProfile,
) -> Result<quinn::Connection, EngineError> {
    Hysteria2Connector::from_udp_profile(server, port, connector_profile.clone())
        .connect_raw_with_udp_profile(&connector_profile)
        .await
}

pub(super) async fn open_udp_packet_path_build(
    build: hysteria2::udp::Hysteria2UdpPacketPathCarrierBuild,
) -> Result<
    (
        quinn::Connection,
        std::sync::Arc<dyn DatagramCodec<zero_core::Address, Error = zero_core::Error>>,
    ),
    EngineError,
> {
    let parts = build.into_connection_parts();
    let (server, port, connector_profile, codec) = parts.into_shared_codec_parts();
    let conn = open_udp_profile_connection(&server, port, connector_profile).await?;
    Ok((conn, codec))
}

impl PacketPathCarrierDescriptorBuild for hysteria2::udp::Hysteria2UdpPacketPathCarrierDescriptor {
    fn into_parts(self) -> (String, String, u16) {
        self.into_parts()
    }
}

impl ManagedDatagramConnectorFlowBuild for hysteria2::udp::Hysteria2UdpConnectorFlow {
    fn into_cache_key(self) -> String {
        self.into_cache_key()
    }
}

pub(super) async fn establish_udp_flow_session(
    endpoint: OutboundEndpoint<'_>,
    target: &zero_core::Address,
    port: u16,
    payload: &[u8],
    resume: hysteria2::udp::Hysteria2UdpFlowResume,
) -> Result<hysteria2::udp::Hysteria2UdpFlowConnection, EngineError> {
    let flow = hysteria2::udp::connector_flow_from_resume(&resume, endpoint.server, endpoint.port);
    let parts = flow.into_connection_parts();
    let connector_profile = parts.into_profile();
    let conn = std::sync::Arc::new(
        open_udp_profile_connection(endpoint.server, endpoint.port, connector_profile).await?,
    );
    Ok(hysteria2::udp::start_udp_flow_with_initial_packet(
        conn, target, port, payload, resume,
    ))
}

pub(super) async fn connect_tcp(
    session: &Session,
    server: &str,
    port: u16,
    password: &str,
    client_fingerprint: Option<&str>,
) -> Result<TcpRelayStream, EngineError> {
    let profile =
        hysteria2::Hysteria2OutboundProfile::from_config_password(password, client_fingerprint);
    let connector = Hysteria2Connector::new(server, port, profile.client_fingerprint());
    let stream = connector.connect(session, &profile).await?;
    Ok(TcpRelayStream::new(stream))
}
