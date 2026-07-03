use alloc::vec::Vec;

use crate::{Address, Error, ProtocolType, SessionAuth};

/// Neutral UDP payload routed by proxy/runtime glue.
///
/// Protocol crates convert this shape into protocol-owned packet models before
/// framing or encryption. Runtime glue should prefer this over storing concrete
/// protocol packet structs in manager state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UdpFlowPacket {
    pub target: Address,
    pub port: u16,
    pub payload: Vec<u8>,
}

/// Neutral inbound UDP request ready for proxy dispatch.
///
/// Protocol crates convert decoded wire packets into this shape before handing
/// them to proxy runtime glue. The proxy should not need to inspect
/// protocol-specific inbound UDP request structs to route a packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboundUdpDispatch {
    target: Address,
    port: u16,
    payload: Vec<u8>,
    protocol: ProtocolType,
    client_session_id: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MuxUdpDecodeFailure {
    Continue,
    End,
}

pub trait MuxUdpResponder: Send {
    fn decode_inbound_dispatch(&mut self, payload: &[u8]) -> Result<InboundUdpDispatch, Error>;

    fn write_response_for_target(
        &mut self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Error>;

    fn end_inbound_stream(&mut self) -> Result<usize, Error>;

    fn decode_failure(&self) -> MuxUdpDecodeFailure {
        MuxUdpDecodeFailure::End
    }
}

pub trait StreamUdpResponder<S>: Send
where
    S: Send,
{
    async fn read_inbound_dispatch(
        &mut self,
        client: &mut S,
    ) -> Result<Option<InboundUdpDispatch>, Error>;

    async fn write_response_for_target(
        &mut self,
        client: &mut S,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Error>;
}

pub trait InboundStreamUdpRelay: Send {
    type Stream: Send;
    type Responder: StreamUdpResponder<Self::Stream>;

    fn into_stream_udp_parts(self) -> (Self::Stream, Self::Responder, Option<SessionAuth>);
}

pub trait DatagramUdpResponder<S>: Send
where
    S: Send,
{
    async fn read_inbound_dispatch(
        &mut self,
        source: &S,
    ) -> Result<Option<InboundUdpDispatch>, Error>;

    fn auth(&self) -> Option<&SessionAuth> {
        None
    }

    fn on_dispatch_success(&mut self, _session_id: u64, _dispatch: &InboundUdpDispatch) {}

    async fn write_response_for_session(
        &mut self,
        source: &S,
        session_id: Option<u64>,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<usize>, Error>;
}

impl InboundUdpDispatch {
    pub fn new(
        protocol: ProtocolType,
        target: Address,
        port: u16,
        payload: Vec<u8>,
        client_session_id: Option<u64>,
    ) -> Self {
        Self {
            target,
            port,
            payload,
            protocol,
            client_session_id,
        }
    }

    pub fn protocol(&self) -> ProtocolType {
        self.protocol
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

    pub fn client_session_id(&self) -> Option<u64> {
        self.client_session_id
    }

    pub fn into_parts(self) -> (ProtocolType, Address, u16, Vec<u8>, Option<u64>) {
        (
            self.protocol,
            self.target,
            self.port,
            self.payload,
            self.client_session_id,
        )
    }
}

impl UdpFlowPacket {
    pub fn new(target: Address, port: u16, payload: Vec<u8>) -> Self {
        Self {
            target,
            port,
            payload,
        }
    }

    pub fn from_parts(target: &Address, port: u16, payload: &[u8]) -> Self {
        Self::new(target.clone(), port, payload.to_vec())
    }

    pub fn into_parts(self) -> (Address, u16, Vec<u8>) {
        (self.target, self.port, self.payload)
    }
}
