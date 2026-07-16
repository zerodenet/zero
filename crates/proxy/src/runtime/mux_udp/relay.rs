use tracing::info;
use zero_core::InboundMuxUdpRelay;

use super::handler::MuxPacketSessionUdpHandler;
use crate::runtime::packet_session_udp::{
    run_packet_session_udp_relay, PacketSessionUdpFailurePolicy, PacketSessionUdpRelayRequest,
};
use crate::runtime::udp_ingress::UdpIngressRuntime;

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
