use zero_core::InboundUdpDispatch;
use zero_core::Session;
use zero_engine::EngineError;

use crate::inbound::stream_udp::{run_stream_udp_relay, StreamUdpRelayRequest, StreamUdpResponder};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

struct VmessStreamUdpResponder {
    inner: vmess::VmessInboundUdpResponder,
    read_buf: Vec<u8>,
}

#[async_trait::async_trait]
impl StreamUdpResponder<TcpRelayStream> for VmessStreamUdpResponder {
    async fn read_inbound_dispatch(
        &mut self,
        client: &mut TcpRelayStream,
    ) -> Result<Option<InboundUdpDispatch>, zero_core::Error> {
        self.inner
            .read_inbound_dispatch_tokio(client, &mut self.read_buf)
            .await
    }

    async fn write_response_for_target(
        &mut self,
        client: &mut TcpRelayStream,
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
    pub(crate) async fn run_vmess_udp_relay(
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
                responder: VmessStreamUdpResponder {
                    inner: vmess::VmessInbound.udp_responder_for(&session),
                    read_buf: vec![0_u8; 64 * 1024],
                },
                session: &session,
                inbound_tag,
                protocol: "vmess_udp",
                auth,
            },
        )
        .await
    }
}
