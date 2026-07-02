use zero_core::Session;
use zero_engine::EngineError;

use super::MieruClientStream;
use crate::runtime::stream_udp::{run_stream_udp_relay, StreamUdpRelayRequest};
use crate::runtime::Proxy;

impl Proxy {
    /// Run a Mieru UDP relay through the generic UDP pipe.
    pub(super) async fn run_mieru_udp_relay(
        &self,
        client: MieruClientStream,
        session: &Session,
        responder: mieru::udp::MieruInboundUdpResponder,
        auth: Option<zero_core::SessionAuth>,
        inbound_tag: &str,
    ) -> Result<(), EngineError> {
        run_stream_udp_relay(
            self,
            StreamUdpRelayRequest {
                client,
                responder,
                session,
                inbound_tag,
                protocol: "mieru_udp",
                auth,
                record_client_io: None,
            },
        )
        .await
    }
}
