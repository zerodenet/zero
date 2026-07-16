use zero_core::{Address, InboundUdpDispatch, ProtocolType, SessionAuth};
use zero_engine::EngineError;

use crate::runtime::udp_dispatch::UdpDispatch;

use super::contract::KernelPipe;

/// Input for one UDP packet dispatch within an inbound UDP association.
pub(crate) struct UdpPipeInput<'a> {
    pub(crate) target: Address,
    pub(crate) port: u16,
    pub(crate) payload: &'a [u8],
    pub(crate) protocol: ProtocolType,
    pub(crate) auth: Option<&'a SessionAuth>,
    /// Per-client-session isolation key (SIP022 3.2.4).
    ///
    /// When `Some`, flows that would collide on `(target, port)` alone are
    /// treated as independent relay sessions. The Shadowsocks 2022 inbound
    /// passes the client's SIP022 session id here; all other protocols pass
    /// `None`.
    pub(crate) client_session_id: Option<u64>,
}

/// UDP datagram pipe.
pub(crate) struct UdpPipe<'a> {
    dispatch: &'a mut UdpDispatch,
}

impl<'a> UdpPipe<'a> {
    pub(crate) fn new(dispatch: &'a mut UdpDispatch) -> Self {
        Self { dispatch }
    }
}

impl KernelPipe for UdpPipe<'_> {
    type Input<'a> = UdpPipeInput<'a>;
    type Output = u64;
    type Error = EngineError;

    async fn dispatch(&mut self, input: Self::Input<'_>) -> Result<Self::Output, Self::Error> {
        UdpDispatch::dispatch(self.dispatch, input).await
    }
}

impl<'a> UdpPipeInput<'a> {
    pub(crate) fn from_inbound_dispatch(
        dispatch: &'a InboundUdpDispatch,
        auth: Option<&'a SessionAuth>,
    ) -> Self {
        Self {
            target: dispatch.target().clone(),
            port: dispatch.port(),
            payload: dispatch.payload(),
            protocol: dispatch.protocol(),
            auth,
            client_session_id: dispatch.client_session_id(),
        }
    }
}
