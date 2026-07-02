use zero_core::Session;
use zero_engine::EngineError;

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

pub(super) async fn connect_tcp(
    session: &Session,
    server: &str,
    port: u16,
    password: &str,
    client_fingerprint: Option<&str>,
) -> Result<TcpRelayStream, EngineError> {
    let profile = hysteria2::outbound_profile_from_config_password(password, client_fingerprint);
    let connector = Hysteria2Connector::new(server, port, profile.client_fingerprint());
    let stream = connector.connect(session, &profile).await?;
    Ok(TcpRelayStream::new(stream))
}
