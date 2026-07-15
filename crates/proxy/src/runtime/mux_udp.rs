use tracing::info;
use zero_core::{InboundMuxUdpReadFailureAction, InboundMuxUdpRelay};

use crate::runtime::packet_session_udp::{
    run_packet_session_udp_relay, PacketSessionUdpFailurePolicy, PacketSessionUdpHandler,
    PacketSessionUdpReadFailure, PacketSessionUdpReadFailureAction, PacketSessionUdpReadResult,
    PacketSessionUdpRelayRequest,
};
use crate::runtime::route_runtime::MuxSubstreamRuntime;
use crate::runtime::udp_ingress::UdpIngressRuntime;

struct MuxPacketSessionUdpHandler<R> {
    relay: R,
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

pub(crate) async fn run_protocol_mux_udp_relay<R>(
    runtime: UdpIngressRuntime,
    relay: R,
    inbound_tag: &str,
    protocol: &'static str,
) where
    R: InboundMuxUdpRelay,
{
    let mux_session_id = relay.mux_session_id();
    let auth = relay.auth().cloned();

    info!(
        inbound_tag = inbound_tag,
        protocol = protocol,
        mux_session_id,
        "mux udp sub-stream started"
    );

    let handler = MuxPacketSessionUdpHandler { relay };

    let _ = run_packet_session_udp_relay(
        runtime,
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

#[cfg(feature = "vmess")]
pub(crate) async fn run_protocol_mux_udp_task<R>(
    runtime: MuxSubstreamRuntime,
    relay: R,
    protocol: &'static str,
) where
    R: InboundMuxUdpRelay,
{
    run_protocol_mux_udp_relay(
        runtime.udp_runtime(),
        relay,
        runtime.inbound_tag(),
        protocol,
    )
    .await;
}

#[cfg(feature = "vless")]
pub(crate) async fn run_protocol_mux_udp_task_with_accept_log<R>(
    runtime: MuxSubstreamRuntime,
    relay: R,
    protocol: &'static str,
    accept_log_message: Option<&'static str>,
) where
    R: InboundMuxUdpRelay,
{
    if let Some(message) = accept_log_message {
        info!(
            inbound_tag = runtime.inbound_tag(),
            network = "udp",
            "{message}"
        );
    }
    run_protocol_mux_udp_relay(
        runtime.udp_runtime(),
        relay,
        runtime.inbound_tag(),
        protocol,
    )
    .await;
}
