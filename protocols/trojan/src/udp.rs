use zero_core::{Address, Error, InboundUdpDispatch, ProtocolType, StreamUdpResponder};
use zero_traits::AsyncSocket;

pub use crate::outbound::TrojanUdpPacket;
pub use crate::outbound::{
    build_udp_request, connector_flow_from_resume, establish_udp_packet_tunnel,
    udp_flow_resume_from_config, TrojanUdpConnectorFlow, TrojanUdpFlowConfig, TrojanUdpFlowIo,
    TrojanUdpFlowResume, TrojanUdpPacketTunnelTarget, TrojanUdpTlsProfile, TrojanUdpTlsProfileSpec,
};

#[cfg(feature = "tokio")]
pub use crate::outbound::{
    establish_udp_flow_with_resume, spawn_udp_flow, TrojanUdpFlowConnection, TrojanUdpFlowHandle,
    TrojanUdpFlowResponseReceiver, TrojanUdpFlowSession, TrojanUdpFlowSessions,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrojanInboundUdpRequest {
    target: Address,
    port: u16,
    payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrojanInboundUdpDispatchParts {
    target: Address,
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

    pub fn pipe_parts(&self) -> (&Address, u16, &[u8], Option<u64>) {
        (
            &self.target,
            self.port,
            &self.payload,
            self.client_session_id,
        )
    }

    pub fn into_pipe_parts(self) -> (Address, u16, Vec<u8>, Option<u64>) {
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

    pub fn target(&self) -> &Address {
        &self.target
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn into_parts(self) -> (Address, u16, Vec<u8>) {
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

impl<S> StreamUdpResponder<S> for TrojanInboundUdpResponder
where
    S: AsyncSocket,
{
    async fn read_inbound_dispatch(
        &mut self,
        client: &mut S,
    ) -> Result<Option<InboundUdpDispatch>, Error> {
        TrojanInboundUdpResponder::read_inbound_dispatch(self, client)
            .await
            .map(Some)
    }

    async fn write_response_for_target(
        &mut self,
        client: &mut S,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Error> {
        TrojanInboundUdpResponder::write_response_for_target(self, client, target, port, payload)
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
        let (target, port, payload) = crate::shared::read_udp_packet(stream).await?;
        Ok(TrojanUdpPacket::new(target, port, payload))
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
        crate::shared::write_udp_packet(stream, target, port, payload).await
    }
}

impl crate::inbound::TrojanInbound {
    pub fn udp_session(&self) -> TrojanInboundUdpSession {
        TrojanInboundUdpSession::new()
    }

    pub fn udp_responder(&self) -> TrojanInboundUdpResponder {
        TrojanInboundUdpResponder::new(self.udp_session())
    }

    pub fn accept_udp_session(&self) -> TrojanInboundUdpResponder {
        self.udp_responder()
    }
}
