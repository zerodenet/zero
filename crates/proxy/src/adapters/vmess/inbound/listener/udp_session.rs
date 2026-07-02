use zero_core::Session;
use zero_engine::EngineError;

use crate::runtime::stream_udp::{run_stream_udp_relay, StreamUdpRelayRequest};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

impl Proxy {
    pub(crate) async fn run_vmess_udp_relay(
        &self,
        client: TcpRelayStream,
        session: Session,
        responder: vmess::VmessInboundUdpResponder,
        auth: Option<zero_core::SessionAuth>,
        inbound_tag: &str,
    ) -> Result<(), EngineError> {
        run_stream_udp_relay(
            self,
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
}
