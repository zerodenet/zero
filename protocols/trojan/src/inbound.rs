//! Trojan inbound protocol handler.

use zero_core::{Address, Error, InboundUdpDispatch, Network, ProtocolType, Session, SessionAuth};
use zero_traits::AsyncSocket;

use super::outbound::TrojanUdpPacket;
use super::shared::{read_password, read_request, CMD_TCP, CMD_UDP};

/// Trojan inbound handler.
#[derive(Debug, Default, Clone, Copy)]
pub struct TrojanInbound;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrojanInboundProfile {
    password: String,
}

impl TrojanInboundProfile {
    pub fn from_config(password: impl Into<String>) -> Self {
        Self {
            password: password.into(),
        }
    }

    pub fn from_config_parts(password: impl Into<String>) -> Self {
        Self::from_config(password)
    }

    pub fn from_config_password(password: impl Into<String>) -> Self {
        Self::from_config_parts(password)
    }

    pub fn inbound_auth(&self) -> SessionAuth {
        TrojanInbound.inbound_auth(self.password.clone())
    }

    pub async fn accept<S: AsyncSocket>(
        &self,
        inbound: TrojanInbound,
        stream: &mut S,
    ) -> Result<TrojanAccept, Error> {
        inbound
            .accept(stream, core::slice::from_ref(&self.password))
            .await
    }
}

/// Result of accepting a Trojan connection.
pub struct TrojanAccept {
    pub session: Session,
    pub command: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrojanInboundSessionKind {
    Tcp,
    Udp,
}

pub fn classify_inbound_session(session: &Session) -> TrojanInboundSessionKind {
    match session.network {
        Network::Udp => TrojanInboundSessionKind::Udp,
        Network::Tcp => TrojanInboundSessionKind::Tcp,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrojanInboundUdpRequest {
    target: zero_core::Address,
    port: u16,
    payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrojanInboundUdpDispatchParts {
    target: zero_core::Address,
    port: u16,
    payload: Vec<u8>,
    client_session_id: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
pub struct TrojanInboundUdpClientResponse<'a> {
    target: &'a Address,
    port: u16,
    payload: &'a [u8],
}

impl<'a> TrojanInboundUdpClientResponse<'a> {
    pub fn new(target: &'a Address, port: u16, payload: &'a [u8]) -> Self {
        Self {
            target,
            port,
            payload,
        }
    }

    pub fn payload_len(&self) -> usize {
        self.payload.len()
    }

    fn target(&self) -> &'a Address {
        self.target
    }

    fn port(&self) -> u16 {
        self.port
    }

    fn payload(&self) -> &'a [u8] {
        self.payload
    }
}

impl TrojanInboundUdpDispatchParts {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Trojan
    }

    pub fn pipe_parts(&self) -> (&zero_core::Address, u16, &[u8], Option<u64>) {
        (
            &self.target,
            self.port,
            &self.payload,
            self.client_session_id,
        )
    }

    pub fn into_pipe_parts(self) -> (zero_core::Address, u16, Vec<u8>, Option<u64>) {
        (self.target, self.port, self.payload, self.client_session_id)
    }

    pub fn into_inbound_dispatch(self) -> InboundUdpDispatch {
        InboundUdpDispatch::new(
            ProtocolType::Trojan,
            self.target,
            self.port,
            self.payload,
            self.client_session_id,
        )
    }
}

impl TrojanInboundUdpRequest {
    fn from_packet(packet: TrojanUdpPacket) -> Self {
        let (target, port, payload) = packet.into_parts();
        Self {
            target,
            port,
            payload,
        }
    }

    pub fn target(&self) -> &zero_core::Address {
        &self.target
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn into_parts(self) -> (zero_core::Address, u16, Vec<u8>) {
        (self.target, self.port, self.payload)
    }

    pub fn into_dispatch_parts(self) -> TrojanInboundUdpDispatchParts {
        let (target, port, payload) = self.into_parts();
        TrojanInboundUdpDispatchParts {
            target,
            port,
            payload,
            client_session_id: None,
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TrojanInboundUdpSession {
    codec: TrojanInboundUdpCodec,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TrojanInboundUdpResponder {
    session: TrojanInboundUdpSession,
}

impl TrojanInboundUdpSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn read_request<S>(&self, stream: &mut S) -> Result<TrojanInboundUdpRequest, Error>
    where
        S: AsyncSocket,
    {
        self.codec
            .read_packet(stream)
            .await
            .map(TrojanInboundUdpRequest::from_packet)
    }

    pub async fn read_dispatch_parts<S>(
        &self,
        stream: &mut S,
    ) -> Result<TrojanInboundUdpDispatchParts, Error>
    where
        S: AsyncSocket,
    {
        self.read_request(stream)
            .await
            .map(TrojanInboundUdpRequest::into_dispatch_parts)
    }

    pub async fn read_inbound_dispatch<S>(
        &self,
        stream: &mut S,
    ) -> Result<InboundUdpDispatch, Error>
    where
        S: AsyncSocket,
    {
        self.read_dispatch_parts(stream)
            .await
            .map(TrojanInboundUdpDispatchParts::into_inbound_dispatch)
    }

    pub async fn write_response<S>(
        &self,
        stream: &mut S,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Error>
    where
        S: AsyncSocket,
    {
        self.codec
            .write_response(stream, target, port, payload)
            .await
    }

    pub async fn write_client_response<S>(
        &self,
        stream: &mut S,
        response: TrojanInboundUdpClientResponse<'_>,
    ) -> Result<usize, Error>
    where
        S: AsyncSocket,
    {
        self.write_response(
            stream,
            response.target(),
            response.port(),
            response.payload(),
        )
        .await
    }

    pub async fn write_client_response_for_target<S>(
        &self,
        stream: &mut S,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Error>
    where
        S: AsyncSocket,
    {
        self.write_client_response(
            stream,
            TrojanInboundUdpClientResponse::new(target, port, payload),
        )
        .await
    }
}

impl TrojanInboundUdpResponder {
    pub fn new(session: TrojanInboundUdpSession) -> Self {
        Self { session }
    }

    pub async fn read_inbound_dispatch<S>(
        &self,
        stream: &mut S,
    ) -> Result<InboundUdpDispatch, Error>
    where
        S: AsyncSocket,
    {
        self.session.read_inbound_dispatch(stream).await
    }

    pub async fn write_response_for_target<S>(
        &self,
        stream: &mut S,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Error>
    where
        S: AsyncSocket,
    {
        self.session
            .write_client_response_for_target(stream, target, port, payload)
            .await
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TrojanInboundUdpCodec;

impl TrojanInboundUdpCodec {
    pub async fn read_packet<S>(&self, stream: &mut S) -> Result<TrojanUdpPacket, Error>
    where
        S: AsyncSocket,
    {
        let (target, port, payload) = super::shared::read_udp_packet(stream).await?;
        Ok(TrojanUdpPacket::new(target, port, payload))
    }

    pub async fn write_response<S>(
        &self,
        stream: &mut S,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Error>
    where
        S: AsyncSocket,
    {
        super::shared::write_udp_packet(stream, target, port, payload).await
    }
}

impl TrojanInbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Trojan
    }

    pub fn inbound_auth(&self, password: impl Into<String>) -> SessionAuth {
        let mut auth = SessionAuth::new("trojan");
        auth.principal_key = Some(password.into());
        auth
    }

    pub fn udp_session(&self) -> TrojanInboundUdpSession {
        TrojanInboundUdpSession::new()
    }

    pub fn udp_responder(&self) -> TrojanInboundUdpResponder {
        TrojanInboundUdpResponder::new(self.udp_session())
    }

    /// Accept a Trojan TCP connection.
    ///
    /// Reads password hash + command + target address from the stream.
    /// The password is validated against `passwords` (hex SHA224 hashes).
    pub async fn accept<S: AsyncSocket>(
        &self,
        stream: &mut S,
        passwords: &[String],
    ) -> Result<TrojanAccept, Error> {
        let hex = read_password(stream).await?;

        // Validate password.
        if !passwords.iter().any(|p| {
            #[cfg(feature = "crypto")]
            {
                use sha2::{Digest, Sha224};
                hex == super::shared::hex::encode(&Sha224::digest(p.as_bytes()))
            }
            #[cfg(not(feature = "crypto"))]
            {
                let _ = p;
                false
            }
        }) {
            return Err(Error::Protocol("trojan: invalid password"));
        }

        let (cmd, addr, port) = read_request(stream).await?;

        let network = match cmd {
            CMD_TCP => Network::Tcp,
            CMD_UDP => Network::Udp,
            _ => return Err(Error::Protocol("trojan: unsupported command")),
        };

        Ok(TrojanAccept {
            session: Session::new(0, addr, port, network, ProtocolType::Trojan),
            command: cmd,
        })
    }
}
