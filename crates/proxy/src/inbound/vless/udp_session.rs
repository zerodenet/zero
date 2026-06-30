use zero_core::{InboundUdpDispatch, Session, SessionAuth};
use zero_engine::EngineError;

use crate::inbound::stream_udp::{run_stream_udp_relay, StreamUdpRelayRequest, StreamUdpResponder};
use crate::runtime::Proxy;
use crate::transport::{ClientStream, MeteredStream};

struct VlessStreamUdpResponder {
    inner: vless::VlessInboundUdpResponder,
    session_id: u64,
}

#[async_trait::async_trait]
impl<S> StreamUdpResponder<MeteredStream<S>> for VlessStreamUdpResponder
where
    S: ClientStream,
{
    async fn read_inbound_dispatch(
        &mut self,
        client: &mut MeteredStream<S>,
    ) -> Result<Option<InboundUdpDispatch>, zero_core::Error> {
        self.inner.read_inbound_dispatch_tokio(client).await
    }

    async fn write_response_for_target(
        &mut self,
        client: &mut MeteredStream<S>,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, zero_core::Error> {
        self.inner
            .write_response_for_target_tokio(client, target, port, payload)
            .await
    }

    fn record_client_io(&mut self, proxy: &Proxy, client: &mut MeteredStream<S>) {
        proxy.record_session_inbound_traffic(self.session_id, client.drain_traffic());
    }
}

impl Proxy {
    pub(crate) async fn handle_vless_udp_session<S>(
        &self,
        mut client: MeteredStream<S>,
        inbound_tag: &str,
        session: Session,
        auth: &Option<SessionAuth>,
    ) -> Result<(), EngineError>
    where
        S: ClientStream,
    {
        vless::VlessInbound.send_response(&mut client).await?;
        self.record_session_inbound_traffic(session.id, client.drain_traffic());

        run_stream_udp_relay(
            self,
            StreamUdpRelayRequest {
                client,
                responder: VlessStreamUdpResponder {
                    inner: vless::VlessInbound.udp_responder(),
                    session_id: session.id,
                },
                session: &session,
                inbound_tag,
                protocol: "vless_udp",
                auth: auth.as_ref(),
            },
        )
        .await
    }
}
