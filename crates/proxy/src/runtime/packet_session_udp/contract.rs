use zero_core::{Address, Error, InboundUdpDispatch, SessionAuth};

pub(crate) struct PacketSessionUdpRelayRequest<'a, H> {
    pub(crate) handler: H,
    pub(crate) inbound_tag: &'a str,
    pub(crate) protocol: &'static str,
    pub(crate) auth: Option<SessionAuth>,
    pub(crate) failure_policy: PacketSessionUdpFailurePolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PacketSessionUdpFailurePolicy {
    ReturnError,
    #[cfg(any(feature = "vless", feature = "vmess"))]
    LogAndBreak,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PacketSessionUdpReadFailureAction {
    #[cfg(any(feature = "vless", feature = "vmess"))]
    Continue,
    End,
}

pub(crate) struct PacketSessionUdpReadFailure {
    pub(crate) error: Error,
    pub(crate) action: PacketSessionUdpReadFailureAction,
}

pub(crate) enum PacketSessionUdpReadResult {
    Dispatch(InboundUdpDispatch),
    End,
}

pub(crate) trait PacketSessionUdpHandler {
    async fn read_inbound_dispatch(
        &mut self,
    ) -> Result<PacketSessionUdpReadResult, PacketSessionUdpReadFailure>;

    async fn write_response_for_target(
        &mut self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Error>;

    async fn finish(&mut self) -> Result<(), Error> {
        Ok(())
    }
}
