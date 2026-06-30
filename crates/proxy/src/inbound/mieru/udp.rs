use zero_core::InboundUdpDispatch;
use zero_core::Session;
use zero_engine::EngineError;

use super::MieruClientStream;
use crate::inbound::stream_udp::{run_stream_udp_relay, StreamUdpRelayRequest, StreamUdpResponder};
use crate::runtime::Proxy;

struct MieruStreamUdpResponder {
    inner: mieru::MieruInboundUdpResponder,
}

#[async_trait::async_trait]
impl StreamUdpResponder<MieruClientStream> for MieruStreamUdpResponder {
    async fn read_inbound_dispatch(
        &mut self,
        client: &mut MieruClientStream,
    ) -> Result<Option<InboundUdpDispatch>, zero_core::Error> {
        self.inner.read_inbound_dispatch_tokio(client).await
    }

    async fn write_response_for_target(
        &mut self,
        client: &mut MieruClientStream,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, zero_core::Error> {
        self.inner
            .write_response_for_target_tokio(client, target, port, payload)
            .await
    }
}

impl Proxy {
    /// Run a Mieru UDP relay through the generic UDP pipe.
    pub(super) async fn run_mieru_udp_relay(
        &self,
        client: MieruClientStream,
        session: &Session,
        inbound_tag: &str,
    ) -> Result<(), EngineError> {
        run_stream_udp_relay(
            self,
            StreamUdpRelayRequest {
                client,
                responder: MieruStreamUdpResponder {
                    inner: mieru::MieruInbound.udp_responder(),
                },
                session,
                inbound_tag,
                protocol: "mieru_udp",
                auth: session.auth.as_ref(),
            },
        )
        .await
    }
}
