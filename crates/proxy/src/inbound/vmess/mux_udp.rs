use tokio::task::JoinSet;
use zero_core::Session;

use crate::inbound::mux_udp::{run_mux_udp_relay, MuxUdpRelayRequest, MuxUdpResponder};
use crate::runtime::Proxy;

struct VmessMuxUdpResponder {
    inner: vmess::VmessInboundMuxUdpResponder,
}

impl MuxUdpResponder for VmessMuxUdpResponder {
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
}

impl Proxy {
    pub(crate) fn spawn_vmess_mux_udp_stream_task(
        &self,
        tasks: &mut JoinSet<()>,
        mux_session_id: u16,
        session: Session,
        up_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
        writer: vmess::mux::VmessInboundMuxWriter,
        inbound_tag: String,
    ) {
        let proxy = self.clone();
        tasks.spawn(async move {
            run_mux_udp_relay(
                &proxy,
                MuxUdpRelayRequest {
                    mux_session_id,
                    up_rx,
                    responder: VmessMuxUdpResponder {
                        inner: vmess::VmessInbound.mux_udp_responder_for(
                            &session,
                            writer,
                            mux_session_id,
                        ),
                    },
                    inbound_tag: &inbound_tag,
                    protocol: "vmess_mux_udp",
                    auth: None,
                },
            )
            .await;
        });
    }
}
