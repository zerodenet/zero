use tokio::sync::mpsc::UnboundedReceiver;
use tracing::info;
use zero_core::{MuxUdpDecodeFailure, MuxUdpResponder, SessionAuth};

use crate::runtime::packet_session_udp::{
    run_packet_session_udp_relay, PacketSessionUdpFailurePolicy, PacketSessionUdpHandler,
    PacketSessionUdpReadFailure, PacketSessionUdpReadFailureAction, PacketSessionUdpReadResult,
    PacketSessionUdpRelayRequest,
};
use crate::runtime::Proxy;

pub(crate) struct MuxUdpRelayRequest<'a, R> {
    pub(crate) mux_session_id: u16,
    pub(crate) up_rx: UnboundedReceiver<Vec<u8>>,
    pub(crate) responder: R,
    pub(crate) inbound_tag: &'a str,
    pub(crate) protocol: &'static str,
    pub(crate) auth: Option<SessionAuth>,
}

struct MuxPacketSessionUdpHandler<R> {
    up_rx: UnboundedReceiver<Vec<u8>>,
    responder: R,
}

impl<R> PacketSessionUdpHandler for MuxPacketSessionUdpHandler<R>
where
    R: MuxUdpResponder,
{
    async fn read_inbound_dispatch(
        &mut self,
    ) -> Result<PacketSessionUdpReadResult, PacketSessionUdpReadFailure> {
        let Some(payload) = self.up_rx.recv().await else {
            return Ok(PacketSessionUdpReadResult::End);
        };
        if payload.is_empty() {
            return Ok(PacketSessionUdpReadResult::End);
        }

        match self.responder.decode_inbound_dispatch(&payload) {
            Ok(inbound_dispatch) => Ok(PacketSessionUdpReadResult::Dispatch(inbound_dispatch)),
            Err(error) => Err(PacketSessionUdpReadFailure {
                error,
                action: match self.responder.decode_failure() {
                    MuxUdpDecodeFailure::Continue => PacketSessionUdpReadFailureAction::Continue,
                    MuxUdpDecodeFailure::End => PacketSessionUdpReadFailureAction::End,
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
        self.responder
            .write_response_for_target(target, port, payload)
    }

    async fn finish(&mut self) -> Result<(), zero_core::Error> {
        self.responder.end_inbound_stream().map(|_| ())
    }
}

pub(crate) async fn run_mux_udp_relay<R>(proxy: &Proxy, request: MuxUdpRelayRequest<'_, R>)
where
    R: MuxUdpResponder,
{
    let MuxUdpRelayRequest {
        mux_session_id,
        up_rx,
        responder,
        inbound_tag,
        protocol,
        auth,
    } = request;

    info!(
        inbound_tag = inbound_tag,
        protocol = protocol,
        mux_session_id,
        "mux udp sub-stream started"
    );

    let _ = mux_session_id;
    let handler = MuxPacketSessionUdpHandler { up_rx, responder };

    let _ = run_packet_session_udp_relay(
        proxy,
        PacketSessionUdpRelayRequest {
            handler,
            inbound_tag,
            protocol,
            auth,
            failure_policy: PacketSessionUdpFailurePolicy::LogAndBreak,
        },
    )
    .await;
}
