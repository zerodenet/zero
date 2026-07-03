use zero_core::Session;
use zero_engine::EngineError;

use crate::runtime::stream_udp::{run_stream_udp_relay, StreamUdpRelayRequest};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

pub(super) async fn run_vmess_udp_relay(
    proxy: &Proxy,
    session: Session,
    relay: vmess::mux::VmessInboundUdpRelay<TcpRelayStream>,
    inbound_tag: &str,
) -> Result<(), EngineError> {
    let (client, responder, auth) = relay.into_parts();
    run_stream_udp_relay(
        proxy,
        StreamUdpRelayRequest {
            client,
            responder,
            session: &session,
            inbound_tag,
            protocol: "vmess_udp",
            auth,
            record_client_io: None,
        },
    )
    .await
}
