use crate::inbound::mux_udp::{
    run_mux_udp_relay, MuxUdpDecodeFailure, MuxUdpRelayRequest, MuxUdpResponder,
};
use crate::runtime::Proxy;

struct VlessMuxUdpResponder {
    inner: vless::VlessInboundMuxUdpResponder,
}

impl MuxUdpResponder for VlessMuxUdpResponder {
    fn decode_inbound_dispatch(
        &mut self,
        payload: &[u8],
    ) -> Result<zero_core::InboundUdpDispatch, zero_core::Error> {
        self.inner.decode_inbound_dispatch(payload)
    }

    fn write_response_for_target(
        &mut self,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, zero_core::Error> {
        self.inner.write_response_for_target(target, port, payload)
    }

    fn end_inbound_stream(&mut self) -> Result<usize, zero_core::Error> {
        self.inner.end_inbound_stream()
    }

    fn decode_failure(&self) -> MuxUdpDecodeFailure {
        MuxUdpDecodeFailure::Continue
    }
}

impl Proxy {
    pub(crate) async fn spawn_vless_mux_udp_stream_task(
        &self,
        mux_session_id: u16,
        up_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
        writer: vless::mux::VlessInboundMuxWriter,
        inbound_tag: &str,
        auth: Option<&zero_core::SessionAuth>,
    ) {
        run_mux_udp_relay(
            self,
            MuxUdpRelayRequest {
                mux_session_id,
                up_rx,
                responder: VlessMuxUdpResponder {
                    inner: vless::VlessInbound.mux_udp_responder(writer, mux_session_id),
                },
                inbound_tag,
                protocol: "vless_mux_udp",
                auth,
            },
        )
        .await;
    }
}
