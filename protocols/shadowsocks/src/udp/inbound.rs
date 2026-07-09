#[cfg(feature = "crypto")]
use alloc::vec::Vec;
#[cfg(feature = "crypto")]
use zero_core::Address;
#[cfg(feature = "crypto")]
use zero_core::{DatagramUdpResponder, Error, InboundUdpDispatch, ProtocolType, SessionAuth};
#[cfg(feature = "crypto")]
use zero_traits::{DatagramCodec, UdpDatagramFraming};

#[cfg(feature = "crypto")]
pub struct ShadowsocksInboundAcceptedUdpSession {
    responder: ShadowsocksInboundUdpResponder,
    auth: SessionAuth,
}

#[cfg(feature = "crypto")]
impl ShadowsocksInboundAcceptedUdpSession {
    pub fn new(responder: ShadowsocksInboundUdpResponder, auth: SessionAuth) -> Self {
        Self { responder, auth }
    }

    pub fn responder(&self) -> &ShadowsocksInboundUdpResponder {
        &self.responder
    }

    pub fn auth(&self) -> &SessionAuth {
        &self.auth
    }

    pub fn into_parts(self) -> (ShadowsocksInboundUdpResponder, SessionAuth) {
        (self.responder, self.auth)
    }

    pub fn into_datagram_relay_parts(self) -> (ShadowsocksInboundUdpResponder, SessionAuth) {
        (self.responder, self.auth)
    }
}

/// Decoded Shadowsocks inbound UDP request.
#[cfg(feature = "crypto")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowsocksInboundUdpPacket {
    target: Address,
    port: u16,
    payload: Vec<u8>,
    /// Flow isolation id for SIP022/2022 UDP sessions. `None` for legacy AEAD.
    client_session_id: Option<u64>,
}

#[cfg(feature = "crypto")]
impl ShadowsocksInboundUdpPacket {
    pub fn target(&self) -> &Address {
        &self.target
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn client_session_id(&self) -> Option<u64> {
        self.client_session_id
    }

    pub fn into_parts(self) -> (Address, u16, Vec<u8>, Option<u64>) {
        (self.target, self.port, self.payload, self.client_session_id)
    }

    pub fn into_dispatch_parts(self) -> ShadowsocksInboundUdpDispatchParts {
        let (target, port, payload, client_session_id) = self.into_parts();
        ShadowsocksInboundUdpDispatchParts {
            target,
            port,
            payload,
            client_session_id,
        }
    }
}

#[cfg(feature = "crypto")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowsocksInboundUdpDispatchParts {
    target: Address,
    port: u16,
    payload: Vec<u8>,
    client_session_id: Option<u64>,
}

#[cfg(feature = "crypto")]
impl ShadowsocksInboundUdpDispatchParts {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Shadowsocks
    }

    pub fn pipe_parts(&self) -> (&Address, u16, &[u8], Option<u64>) {
        (
            &self.target,
            self.port,
            &self.payload,
            self.client_session_id,
        )
    }

    pub fn into_parts(self) -> (Address, u16, Vec<u8>, Option<u64>) {
        (self.target, self.port, self.payload, self.client_session_id)
    }

    pub fn into_inbound_dispatch(self) -> InboundUdpDispatch {
        InboundUdpDispatch::new(
            ProtocolType::Shadowsocks,
            self.target,
            self.port,
            self.payload,
            self.client_session_id,
        )
    }
}

#[cfg(feature = "crypto")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowsocksInboundUdpResponse {
    datagram: Vec<u8>,
}

#[cfg(feature = "crypto")]
impl ShadowsocksInboundUdpResponse {
    pub fn datagram(&self) -> &[u8] {
        &self.datagram
    }

    pub fn into_datagram(self) -> Vec<u8> {
        self.datagram
    }
}

#[cfg(feature = "crypto")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowsocksInboundUdpResponseTarget {
    client_session_id: Option<u64>,
    target: Address,
    port: u16,
}

#[cfg(feature = "crypto")]
impl ShadowsocksInboundUdpResponseTarget {
    pub fn new(client_session_id: Option<u64>, target: Address, port: u16) -> Self {
        Self {
            client_session_id,
            target,
            port,
        }
    }

    pub fn from_parts(client_session_id: Option<u64>, target: &Address, port: u16) -> Self {
        Self::new(client_session_id, target.clone(), port)
    }
}

#[cfg(feature = "crypto")]
pub struct ShadowsocksInboundUdpClientResponse<'a> {
    target: &'a Address,
    port: u16,
    payload: &'a [u8],
}

#[cfg(feature = "crypto")]
impl<'a> ShadowsocksInboundUdpClientResponse<'a> {
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

/// Protocol-owned codec/state for Shadowsocks inbound UDP.
///
/// Runtime code owns socket I/O and routing, while this type owns
/// Shadowsocks UDP decoding, SIP022 replay protection, and response encoding.
#[cfg(feature = "crypto")]
pub struct ShadowsocksInboundUdpCodec {
    cipher: crate::shared::CipherKind,
    password: Vec<u8>,
    #[cfg(feature = "blake3")]
    replay_windows: std::collections::HashMap<u64, crate::shared::ReplayWindow>,
}

#[cfg(feature = "crypto")]
impl ShadowsocksInboundUdpCodec {
    pub fn new(cipher: crate::shared::CipherKind, password: &[u8]) -> Self {
        Self {
            cipher,
            password: password.to_vec(),
            #[cfg(feature = "blake3")]
            replay_windows: std::collections::HashMap::new(),
        }
    }

    pub fn decode_request(
        &mut self,
        datagram: &[u8],
    ) -> Result<ShadowsocksInboundUdpPacket, Error> {
        if self.cipher.is_blake3() {
            #[cfg(feature = "blake3")]
            {
                let (target, port, payload, client_session_id, packet_id) =
                    crate::shared::decode_udp_datagram_2022_session(
                        self.cipher,
                        &self.password,
                        datagram,
                    )?;
                if !self
                    .replay_windows
                    .entry(client_session_id)
                    .or_default()
                    .check_and_update(packet_id)
                {
                    return Err(Error::Protocol("ss: udp replay rejected"));
                }
                return Ok(ShadowsocksInboundUdpPacket {
                    target,
                    port,
                    payload,
                    client_session_id: Some(client_session_id),
                });
            }
            #[cfg(not(feature = "blake3"))]
            return Err(Error::Protocol(
                "ss: 2022 udp decode requires `blake3` feature",
            ));
        }

        let codec = crate::outbound::ShadowsocksDatagramCodec {
            cipher: self.cipher,
            password: self.password.clone(),
        };
        let (target, port, payload) = codec
            .decode(datagram)
            .ok_or(Error::Protocol("ss: udp datagram decode failed"))?;
        Ok(ShadowsocksInboundUdpPacket {
            target,
            port,
            payload,
            client_session_id: None,
        })
    }

    pub fn encode_response(
        &self,
        client_session_id: Option<u64>,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, Error> {
        if self.cipher.is_blake3() {
            #[cfg(feature = "blake3")]
            {
                return crate::shared::encode_udp_response_2022(
                    self.cipher,
                    &self.password,
                    client_session_id.unwrap_or(0),
                    target,
                    port,
                    payload,
                );
            }
            #[cfg(not(feature = "blake3"))]
            return Err(Error::Protocol(
                "ss: 2022 udp encode requires `blake3` feature",
            ));
        }

        <crate::outbound::ShadowsocksOutbound as UdpDatagramFraming<
            crate::outbound::ShadowsocksUdpPacketTarget<'_>,
            crate::outbound::ShadowsocksUdpDecodeContext<'_>,
        >>::encode_udp_datagram(
            &crate::outbound::ShadowsocksOutbound,
            &crate::outbound::ShadowsocksUdpPacketTarget {
                target,
                port,
                payload,
                cipher: self.cipher,
                password: &self.password,
            },
        )
    }

    pub fn encode_response_to_client(
        &self,
        client_session_id: Option<u64>,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<ShadowsocksInboundUdpResponse, Error> {
        Ok(ShadowsocksInboundUdpResponse {
            datagram: self.encode_response(client_session_id, target, port, payload)?,
        })
    }

    pub fn response_frame(
        &self,
        target: &ShadowsocksInboundUdpResponseTarget,
        payload: &[u8],
    ) -> Result<ShadowsocksInboundUdpResponse, Error> {
        self.encode_response_to_client(
            target.client_session_id,
            &target.target,
            target.port,
            payload,
        )
    }
}

#[cfg(feature = "crypto")]
pub struct ShadowsocksInboundUdpSession {
    codec: ShadowsocksInboundUdpCodec,
    proxy_sessions: std::collections::HashMap<u64, Option<u64>>,
    proxy_clients: std::collections::HashMap<u64, std::net::SocketAddr>,
}

#[cfg(feature = "crypto")]
pub struct ShadowsocksInboundUdpResponder {
    session: ShadowsocksInboundUdpSession,
    pending_client: Option<std::net::SocketAddr>,
    read_buf: Vec<u8>,
}

#[cfg(feature = "crypto")]
impl ShadowsocksInboundUdpSession {
    pub fn new(codec: ShadowsocksInboundUdpCodec) -> Self {
        Self {
            codec,
            proxy_sessions: std::collections::HashMap::new(),
            proxy_clients: std::collections::HashMap::new(),
        }
    }

    pub fn decode_request(
        &mut self,
        datagram: &[u8],
    ) -> Result<ShadowsocksInboundUdpPacket, Error> {
        self.codec.decode_request(datagram)
    }

    pub fn decode_dispatch_parts(
        &mut self,
        datagram: &[u8],
    ) -> Result<ShadowsocksInboundUdpDispatchParts, Error> {
        self.decode_request(datagram)
            .map(ShadowsocksInboundUdpPacket::into_dispatch_parts)
    }

    pub fn decode_inbound_dispatch(
        &mut self,
        datagram: &[u8],
    ) -> Result<InboundUdpDispatch, Error> {
        self.decode_dispatch_parts(datagram)
            .map(ShadowsocksInboundUdpDispatchParts::into_inbound_dispatch)
    }

    pub fn encode_response_to_client(
        &self,
        client_session_id: Option<u64>,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<ShadowsocksInboundUdpResponse, Error> {
        self.codec
            .encode_response_to_client(client_session_id, target, port, payload)
    }

    pub fn response_frame(
        &self,
        target: &ShadowsocksInboundUdpResponseTarget,
        payload: &[u8],
    ) -> Result<ShadowsocksInboundUdpResponse, Error> {
        self.codec.response_frame(target, payload)
    }

    pub fn response_frame_for_proxy_session(
        &self,
        proxy_session_id: u64,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<ShadowsocksInboundUdpResponse, Error> {
        let response_target =
            self.response_target_for_proxy_session(proxy_session_id, target, port);
        self.response_frame(&response_target, payload)
    }

    pub fn response_datagram_for_proxy_session(
        &self,
        proxy_session_id: u64,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<ShadowsocksInboundUdpResponseDatagram, Error> {
        let datagram =
            self.response_frame_for_proxy_session(proxy_session_id, target, port, payload)?;
        Ok(ShadowsocksInboundUdpResponseDatagram {
            datagram: datagram.into_datagram(),
        })
    }

    pub async fn send_response_to_client_tokio(
        &self,
        socket: &tokio::net::UdpSocket,
        proxy_session_id: u64,
        target: &Address,
        port: u16,
        payload: &[u8],
        client: std::net::SocketAddr,
    ) -> Result<usize, Error> {
        let datagram =
            self.response_datagram_for_proxy_session(proxy_session_id, target, port, payload)?;
        socket
            .send_to(datagram.as_slice(), client)
            .await
            .map_err(|_| Error::Io("failed to send Shadowsocks UDP response"))
    }

    pub async fn send_client_response_to_client_tokio(
        &self,
        socket: &tokio::net::UdpSocket,
        proxy_session_id: u64,
        response: ShadowsocksInboundUdpClientResponse<'_>,
        client: std::net::SocketAddr,
    ) -> Result<usize, Error> {
        self.send_response_to_client_tokio(
            socket,
            proxy_session_id,
            response.target(),
            response.port(),
            response.payload(),
            client,
        )
        .await
    }

    fn record_proxy_session(&mut self, proxy_session_id: u64, client_session_id: Option<u64>) {
        if client_session_id.is_some() {
            self.proxy_sessions
                .insert(proxy_session_id, client_session_id);
        }
    }

    fn record_client_session(
        &mut self,
        proxy_session_id: u64,
        client_session_id: Option<u64>,
        client: std::net::SocketAddr,
    ) {
        self.record_proxy_session(proxy_session_id, client_session_id);
        self.proxy_clients.insert(proxy_session_id, client);
    }

    pub fn record_dispatch_success(
        &mut self,
        proxy_session_id: u64,
        client_session_id: Option<u64>,
        client: std::net::SocketAddr,
    ) {
        self.record_client_session(proxy_session_id, client_session_id, client);
    }

    pub async fn send_proxy_session_response_to_client_tokio(
        &self,
        socket: &tokio::net::UdpSocket,
        proxy_session_id: u64,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<usize>, Error> {
        let Some(&client) = self.proxy_clients.get(&proxy_session_id) else {
            return Ok(None);
        };
        self.send_response_to_client_tokio(socket, proxy_session_id, target, port, payload, client)
            .await
            .map(Some)
    }

    pub async fn send_proxy_session_client_response_to_client_tokio(
        &self,
        socket: &tokio::net::UdpSocket,
        proxy_session_id: u64,
        response: ShadowsocksInboundUdpClientResponse<'_>,
    ) -> Result<Option<usize>, Error> {
        let Some(&client) = self.proxy_clients.get(&proxy_session_id) else {
            return Ok(None);
        };
        self.send_client_response_to_client_tokio(socket, proxy_session_id, response, client)
            .await
            .map(Some)
    }

    pub async fn send_client_response_for_proxy_session_to_client_tokio(
        &self,
        socket: &tokio::net::UdpSocket,
        proxy_session_id: Option<u64>,
        response: ShadowsocksInboundUdpClientResponse<'_>,
    ) -> Result<Option<usize>, Error> {
        let Some(proxy_session_id) = proxy_session_id else {
            return Ok(None);
        };
        self.send_proxy_session_client_response_to_client_tokio(socket, proxy_session_id, response)
            .await
    }

    pub async fn send_response_for_target_proxy_session_to_client_tokio(
        &self,
        socket: &tokio::net::UdpSocket,
        proxy_session_id: Option<u64>,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<usize>, Error> {
        let Some(proxy_session_id) = proxy_session_id else {
            return Ok(None);
        };
        self.send_proxy_session_response_to_client_tokio(
            socket,
            proxy_session_id,
            target,
            port,
            payload,
        )
        .await
    }

    pub fn response_target_for_proxy_session(
        &self,
        proxy_session_id: u64,
        target: &Address,
        port: u16,
    ) -> ShadowsocksInboundUdpResponseTarget {
        ShadowsocksInboundUdpResponseTarget::from_parts(
            self.proxy_sessions
                .get(&proxy_session_id)
                .copied()
                .flatten(),
            target,
            port,
        )
    }
}

#[cfg(feature = "crypto")]
impl ShadowsocksInboundUdpResponder {
    pub fn new(session: ShadowsocksInboundUdpSession) -> Self {
        Self {
            session,
            pending_client: None,
            read_buf: vec![0_u8; 64 * 1024],
        }
    }

    pub fn decode_inbound_dispatch(
        &mut self,
        datagram: &[u8],
    ) -> Result<InboundUdpDispatch, Error> {
        self.session.decode_inbound_dispatch(datagram)
    }

    pub async fn read_inbound_dispatch_from_socket_tokio(
        &mut self,
        socket: &tokio::net::UdpSocket,
    ) -> Result<InboundUdpDispatch, Error> {
        loop {
            let (n, client) = socket
                .recv_from(&mut self.read_buf)
                .await
                .map_err(|_| Error::Io("ss udp recv error"))?;
            let dispatch = {
                let buf = &self.read_buf[..n];
                self.session.decode_inbound_dispatch(buf)
            };
            match dispatch {
                Ok(dispatch) => {
                    self.pending_client = Some(client);
                    return Ok(dispatch);
                }
                Err(_) => continue,
            }
        }
    }

    pub fn record_dispatch_success(
        &mut self,
        proxy_session_id: u64,
        client_session_id: Option<u64>,
        client: std::net::SocketAddr,
    ) {
        self.session
            .record_dispatch_success(proxy_session_id, client_session_id, client);
    }

    pub fn record_pending_dispatch_success(
        &mut self,
        proxy_session_id: u64,
        client_session_id: Option<u64>,
    ) {
        if let Some(client) = self.pending_client.take() {
            self.record_dispatch_success(proxy_session_id, client_session_id, client);
        }
    }

    pub async fn send_response_for_target_proxy_session_to_client_tokio(
        &self,
        socket: &tokio::net::UdpSocket,
        proxy_session_id: Option<u64>,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<usize>, Error> {
        self.session
            .send_response_for_target_proxy_session_to_client_tokio(
                socket,
                proxy_session_id,
                target,
                port,
                payload,
            )
            .await
    }
}

#[cfg(feature = "crypto")]
#[async_trait::async_trait]
impl DatagramUdpResponder<std::sync::Arc<tokio::net::UdpSocket>>
    for ShadowsocksInboundUdpResponder
{
    async fn read_inbound_dispatch(
        &mut self,
        socket: &std::sync::Arc<tokio::net::UdpSocket>,
    ) -> Result<Option<InboundUdpDispatch>, Error> {
        self.read_inbound_dispatch_from_socket_tokio(socket.as_ref())
            .await
            .map(Some)
    }

    fn on_dispatch_success(&mut self, session_id: u64, dispatch: &InboundUdpDispatch) {
        self.record_pending_dispatch_success(session_id, dispatch.client_session_id());
    }

    async fn write_response_for_session(
        &mut self,
        socket: &std::sync::Arc<tokio::net::UdpSocket>,
        session_id: Option<u64>,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<usize>, Error> {
        self.send_response_for_target_proxy_session_to_client_tokio(
            socket.as_ref(),
            session_id,
            target,
            port,
            payload,
        )
        .await
    }
}

#[cfg(feature = "crypto")]
#[derive(Debug)]
pub struct ShadowsocksInboundUdpResponseDatagram {
    datagram: Vec<u8>,
}

#[cfg(feature = "crypto")]
impl ShadowsocksInboundUdpResponseDatagram {
    pub fn len(&self) -> usize {
        self.datagram.len()
    }

    pub fn is_empty(&self) -> bool {
        self.datagram.is_empty()
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.datagram
    }

    pub fn into_datagram(self) -> Vec<u8> {
        self.datagram
    }
}
