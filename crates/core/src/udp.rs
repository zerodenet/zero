use alloc::vec::Vec;

use crate::Address;

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
