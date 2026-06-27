//! Hysteria2 outbound — TCP connect.
//!
//! TCP outbound connect ([`connect_tcp`]) moved here from `runtime/upstream.rs`
//! so the runtime dispatches via registered TCP outbound capabilities. UDP datagram
//! management lives in the Hysteria2 adapter UDP module.

use zero_core::Session;
use zero_engine::EngineError;

use crate::runtime::Proxy;
use crate::transport::{Hysteria2Stream, QuicConnectionOptions, TcpRelayStream};

pub(crate) struct Hysteria2Connector {
    server: String,
    port: u16,
    password: String,
    client_fingerprint: Option<String>,
}

impl Hysteria2Connector {
    pub(crate) fn new(server: &str, port: u16, password: &str) -> Self {
        Self {
            server: server.to_owned(),
            port,
            password: password.to_owned(),
            client_fingerprint: None,
        }
    }

    pub(crate) fn with_fingerprint(mut self, fingerprint: Option<&str>) -> Self {
        self.client_fingerprint = fingerprint.map(ToOwned::to_owned);
        self
    }

    pub(crate) fn from_udp_profile(
        server: &str,
        port: u16,
        profile: hysteria2::Hysteria2UdpConnectorProfile,
    ) -> Self {
        Self {
            server: server.to_owned(),
            port,
            password: String::new(),
            client_fingerprint: profile.client_fingerprint().map(ToOwned::to_owned),
        }
    }

    pub(crate) async fn connect_raw(&self) -> Result<quinn::Connection, EngineError> {
        let conn = self.open_quic_connection().await?;

        let (send, recv) = conn.open_bi().await.map_err(|error| {
            EngineError::Io(std::io::Error::other(format!("hysteria2 open_bi: {error}")))
        })?;
        let mut stream = Hysteria2Stream::new(send, recv);
        authenticate_with_password(&conn, &mut stream, &self.password).await?;

        Ok(conn)
    }

    async fn open_quic_connection(&self) -> Result<quinn::Connection, EngineError> {
        crate::transport::open_hysteria2_quic_connection(QuicConnectionOptions {
            server: &self.server,
            port: self.port,
            alpn: vec![b"hysteria2".to_vec()],
            client_fingerprint: self.client_fingerprint.as_deref(),
            datagram_receive_buffer_size: Some(65536),
        })
        .await
    }

    pub(crate) async fn connect_raw_with_udp_profile(
        &self,
        profile: &hysteria2::Hysteria2UdpConnectorProfile,
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

    pub(crate) async fn connect(&self, session: &Session) -> Result<Hysteria2Stream, EngineError> {
        let conn = self.connect_raw().await?;
        let (send, recv) = conn.open_bi().await.map_err(|error| {
            EngineError::Io(std::io::Error::other(format!("hysteria2 open_bi: {error}")))
        })?;

        let mut stream = Hysteria2Stream::new(send, recv);
        hysteria2::Hysteria2Outbound
            .send_tcp_connect(&mut stream, session)
            .await
            .map_err(EngineError::Core)?;
        hysteria2::Hysteria2Outbound
            .read_connect_response(&mut stream)
            .await
            .map_err(EngineError::Core)?;

        Ok(stream)
    }
}

async fn authenticate_with_password(
    conn: &quinn::Connection,
    stream: &mut Hysteria2Stream,
    password: &str,
) -> Result<(), EngineError> {
    let mut salt = [0u8; 32];
    conn.export_keying_material(&mut salt, b"hysteria2 auth", &[])
        .map_err(|_| EngineError::Io(std::io::Error::other("hysteria2 key export failed")))?;

    hysteria2::Hysteria2Outbound
        .authenticate_with_salt(stream, password, &salt)
        .await
        .map_err(EngineError::Core)
}

/// Establish a Hysteria2 TCP upstream via QUIC.
///
/// Moved from `runtime/upstream.rs`. The runtime dispatches via the adapter
/// trait instead of a per-protocol `connect_via_*` method.
pub(crate) async fn connect_tcp(
    _proxy: &Proxy,
    session: &Session,
    server: &str,
    port: u16,
    password: &str,
    client_fingerprint: Option<&str>,
) -> Result<TcpRelayStream, EngineError> {
    let connector =
        Hysteria2Connector::new(server, port, password).with_fingerprint(client_fingerprint);
    let stream = connector.connect(session).await?;
    Ok(TcpRelayStream::new(stream))
}
