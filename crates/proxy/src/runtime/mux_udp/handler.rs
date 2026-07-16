use zero_core::{InboundMuxUdpReadFailureAction, InboundMuxUdpRelay};

use crate::runtime::packet_session_udp::{
    PacketSessionUdpHandler, PacketSessionUdpReadFailure, PacketSessionUdpReadFailureAction,
    PacketSessionUdpReadResult,
};

pub(super) struct MuxPacketSessionUdpHandler<R> {
    pub(super) relay: R,
}

impl<R> PacketSessionUdpHandler for MuxPacketSessionUdpHandler<R>
where
    R: InboundMuxUdpRelay,
{
    async fn read_inbound_dispatch(
        &mut self,
    ) -> Result<PacketSessionUdpReadResult, PacketSessionUdpReadFailure> {
        match self.relay.read_inbound_dispatch().await {
            Ok(Some(inbound_dispatch)) => {
                Ok(PacketSessionUdpReadResult::Dispatch(inbound_dispatch))
            }
            Ok(None) => Ok(PacketSessionUdpReadResult::End),
            Err(failure) => Err(PacketSessionUdpReadFailure {
                error: failure.error,
                action: match failure.action {
                    InboundMuxUdpReadFailureAction::Continue => {
                        PacketSessionUdpReadFailureAction::Continue
                    }
                    InboundMuxUdpReadFailureAction::End => PacketSessionUdpReadFailureAction::End,
                },
            }),
        }
    }

    async fn write_response_for_target(
        &mut self,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, zero_core::Error> {
        self.relay.write_response_for_target(target, port, payload)
    }

    async fn finish(&mut self) -> Result<(), zero_core::Error> {
        self.relay.end_inbound_stream().map(|_| ())
    }
}
