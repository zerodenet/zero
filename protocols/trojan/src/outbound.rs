//! Trojan outbound protocol handler.

use std::string::String;

use zero_core::{Address, Error, ProtocolType, Session};
use zero_traits::{AsyncSocket, TcpTunnelProtocol};

use super::shared::CMD_TCP;

/// Trojan outbound handler.
#[derive(Debug, Default, Clone, Copy)]
pub struct TrojanOutbound;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrojanTcpOutboundProfile {
    password: String,
}

impl TrojanTcpOutboundProfile {
    pub fn from_config_parts(password: impl Into<String>) -> Self {
        Self {
            password: password.into(),
        }
    }

    pub fn from_config_password(password: &str) -> Self {
        Self::from_config_parts(password)
    }

    pub async fn establish_tcp_tunnel<S>(
        &self,
        stream: &mut S,
        session: &Session,
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        TrojanOutbound
            .establish_tcp_tunnel(stream, &TrojanTcpTunnelTarget::new(session, &self.password))
            .await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrojanTcpTlsProfile {
    server_name: Option<String>,
    insecure: bool,
    client_fingerprint: Option<String>,
}

impl TrojanTcpTlsProfile {
    pub fn from_config_parts(
        sni: Option<&str>,
        insecure: bool,
        client_fingerprint: Option<&str>,
    ) -> Self {
        Self {
            server_name: sni.map(ToOwned::to_owned),
            insecure,
            client_fingerprint: client_fingerprint.map(ToOwned::to_owned),
        }
    }

    pub fn server_name(&self) -> Option<&str> {
        self.server_name.as_deref()
    }

    pub fn insecure(&self) -> bool {
        self.insecure
    }

    pub fn client_fingerprint(&self) -> Option<&str> {
        self.client_fingerprint.as_deref()
    }
}

pub fn tcp_outbound_profile_from_config_password(password: &str) -> TrojanTcpOutboundProfile {
    TrojanTcpOutboundProfile::from_config_password(password)
}

pub fn tcp_tls_profile_from_config(
    sni: Option<&str>,
    insecure: bool,
    client_fingerprint: Option<&str>,
) -> TrojanTcpTlsProfile {
    TrojanTcpTlsProfile::from_config_parts(sni, insecure, client_fingerprint)
}

impl TrojanOutbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Trojan
    }

    /// Send the Trojan request over an established TLS stream.
    ///
    /// Writes: password hash + CRLF + CMD + address + port + CRLF.
    /// The upstream server then connects to the target and relays data.
    pub async fn send_request<S: AsyncSocket>(
        &self,
        stream: &mut S,
        session: &Session,
        password: &str,
    ) -> Result<(), Error> {
        let request = build_tcp_request(password, &session.target, session.port)?;
        stream
            .write_all(&request)
            .await
            .map_err(|_| Error::Io("trojan: write failed"))
    }
}

/// Target parameters for Trojan TCP tunnel.
#[derive(Debug, Clone, Copy)]
pub struct TrojanTcpTunnelTarget<'a> {
    pub session: &'a Session,
    pub password: &'a str,
}

impl<'a> TrojanTcpTunnelTarget<'a> {
    pub fn new(session: &'a Session, password: &'a str) -> Self {
        Self { session, password }
    }
}

impl<'a> TcpTunnelProtocol<TrojanTcpTunnelTarget<'a>> for TrojanOutbound {
    type Error = Error;

    async fn establish_tcp_tunnel<S>(
        &self,
        stream: &mut S,
        target: &TrojanTcpTunnelTarget<'a>,
    ) -> Result<(), Self::Error>
    where
        S: AsyncSocket,
    {
        self.send_request(stream, target.session, target.password)
            .await
    }
}

fn build_tcp_request(password: &str, addr: &Address, port: u16) -> Result<Vec<u8>, Error> {
    super::shared::build_request(password, addr, port, CMD_TCP)
}
