use zero_core::Session;
use zero_engine::EngineError;

use crate::runtime::stream_udp::{run_stream_udp_relay, StreamUdpRelayRequest};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

pub(super) async fn run_trojan_udp_relay(
    proxy: &Proxy,
    client: TcpRelayStream,
    session: Session,
    responder: trojan::udp::TrojanInboundUdpResponder,
    auth: Option<zero_core::SessionAuth>,
    inbound_tag: &str,
) -> Result<(), EngineError> {
    run_stream_udp_relay(
        proxy,
        StreamUdpRelayRequest {
            client,
            responder,
            session: &session,
            inbound_tag,
            protocol: "trojan_udp",
            auth,
            record_client_io: None,
        },
    )
    .await
}
