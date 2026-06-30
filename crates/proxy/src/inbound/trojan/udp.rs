use zero_core::InboundUdpDispatch;
use zero_core::Session;
use zero_engine::EngineError;

use crate::inbound::stream_udp::{run_stream_udp_relay, StreamUdpRelayRequest, StreamUdpResponder};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

struct TrojanStreamUdpResponder {
    inner: trojan::TrojanInboundUdpResponder,
}

#[async_trait::async_trait]
impl StreamUdpResponder<TcpRelayStream> for TrojanStreamUdpResponder {
    async fn read_inbound_dispatch(
        &mut self,
        client: &mut TcpRelayStream,
    ) -> Result<Option<InboundUdpDispatch>, zero_core::Error> {
        self.inner.read_inbound_dispatch(client).await.map(Some)
    }

    async fn write_response_for_target(
        &mut self,
        client: &mut TcpRelayStream,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, zero_core::Error> {
        self.inner
            .write_response_for_target(client, target, port, payload)
            .await
    }
}

impl Proxy {
    pub(super) async fn run_trojan_udp_relay(
        &self,
        client: TcpRelayStream,
        session: Session,
        inbound_tag: &str,
    ) -> Result<(), EngineError> {
        let auth = session.auth.as_ref();
        run_stream_udp_relay(
            self,
            StreamUdpRelayRequest {
                client,
                responder: TrojanStreamUdpResponder {
                    inner: trojan::TrojanInbound.udp_responder(),
                },
                session: &session,
                inbound_tag,
                protocol: "trojan_udp",
                auth,
            },
        )
        .await
    }
}
