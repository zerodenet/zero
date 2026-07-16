use zero_core::StreamUdpResponder;

use super::recording::record_stream_udp_client_io;
use crate::runtime::packet_session_udp::{
    PacketSessionUdpHandler, PacketSessionUdpReadFailure, PacketSessionUdpReadFailureAction,
    PacketSessionUdpReadResult,
};
use crate::runtime::udp_ingress::UdpIngressRuntime;

pub(super) struct StreamPacketSessionUdpHandler<S, R> {
    pub(super) runtime: UdpIngressRuntime,
    pub(super) client: S,
    pub(super) responder: R,
    pub(super) stream_session_id: u64,
    pub(super) record_client_io: Option<fn(&UdpIngressRuntime, u64, &mut S)>,
}

impl<S, R> PacketSessionUdpHandler for StreamPacketSessionUdpHandler<S, R>
where
    S: Send,
    R: StreamUdpResponder<S>,
{
    async fn read_inbound_dispatch(
        &mut self,
    ) -> Result<PacketSessionUdpReadResult, PacketSessionUdpReadFailure> {
        match self.responder.read_inbound_dispatch(&mut self.client).await {
            Ok(Some(inbound_dispatch)) => {
                record_stream_udp_client_io(
                    &self.runtime,
                    self.record_client_io,
                    self.stream_session_id,
                    &mut self.client,
                );
                Ok(PacketSessionUdpReadResult::Dispatch(inbound_dispatch))
            }
            Ok(None) => Ok(PacketSessionUdpReadResult::End),
            Err(error) => Err(PacketSessionUdpReadFailure {
                error,
                action: PacketSessionUdpReadFailureAction::End,
            }),
        }
    }

    async fn write_response_for_target(
        &mut self,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, zero_core::Error> {
        let written = self
            .responder
            .write_response_for_target(&mut self.client, target, port, payload)
            .await?;
        record_stream_udp_client_io(
            &self.runtime,
            self.record_client_io,
            self.stream_session_id,
            &mut self.client,
        );
        Ok(written)
    }
}
