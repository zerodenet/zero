use zero_core::Session;
use zero_engine::EngineError;

use crate::runtime::stream_udp::{run_stream_udp_relay, StreamUdpRelayRequest};
use crate::runtime::Proxy;
use crate::transport::{ClientStream, MeteredStream};

fn record_metered_client_io<S>(proxy: &Proxy, session_id: u64, client: &mut MeteredStream<S>)
where
    S: ClientStream,
{
    proxy.record_session_inbound_traffic(session_id, client.drain_traffic());
}

impl Proxy {
    pub(crate) async fn handle_vless_udp_session<S>(
        &self,
        client: MeteredStream<S>,
        inbound_tag: &str,
        session: Session,
        responder: vless::VlessInboundUdpResponder,
        auth: Option<zero_core::SessionAuth>,
    ) -> Result<(), EngineError>
    where
        S: ClientStream,
    {
        run_stream_udp_relay(
            self,
            StreamUdpRelayRequest {
                client,
                responder,
                session: &session,
                inbound_tag,
                protocol: "vless_udp",
                auth,
                record_client_io: Some(record_metered_client_io::<S>),
            },
        )
        .await
    }
}
