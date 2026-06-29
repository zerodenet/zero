use alloc::vec::Vec;

use crate::{Address, ProtocolType};

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
