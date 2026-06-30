use std::sync::Arc;

use zero_core::InboundUdpDispatch;
use zero_engine::EngineError;

use crate::inbound::datagram_udp::{
    run_datagram_udp_relay, DatagramUdpRelayRequest, DatagramUdpResponder,
};
use crate::runtime::Proxy;

struct Hysteria2DatagramUdpResponder {
    inner: hysteria2::Hysteria2InboundUdpResponder,
}

#[async_trait::async_trait]
impl DatagramUdpResponder<Arc<quinn::Connection>> for Hysteria2DatagramUdpResponder {
    async fn read_inbound_dispatch(
        &mut self,
        conn: &Arc<quinn::Connection>,
    ) -> Result<Option<InboundUdpDispatch>, zero_core::Error> {
        self.inner
            .read_inbound_dispatch_from_datagram(conn)
            .await
            .map(Some)
    }

    fn on_dispatch_success(&mut self, session_id: u64, _dispatch: &InboundUdpDispatch) {
        self.inner.record_pending_dispatch_success(session_id);
    }

    async fn write_response_for_session(
        &mut self,
        conn: &Arc<quinn::Connection>,
        session_id: Option<u64>,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<usize>, zero_core::Error> {
        self.inner
            .send_response_for_target_proxy_session(conn, session_id, target, port, payload)
    }
}

impl Proxy {
    pub(super) async fn hysteria2_datagram_loop(
        conn: Arc<quinn::Connection>,
        inbound_tag: String,
        proxy: Proxy,
    ) -> Result<(), EngineError> {
        run_datagram_udp_relay(
            &proxy,
            DatagramUdpRelayRequest {
                source: conn,
                responder: Hysteria2DatagramUdpResponder {
                    inner: hysteria2::Hysteria2Inbound.udp_responder(),
                },
                inbound_tag: &inbound_tag,
                poll_upstream: true,
            },
        )
        .await
    }
}
