use zero_core::Session;
use zero_engine::EngineError;

use crate::runtime::stream_udp::run_protocol_stream_udp_relay;
use crate::runtime::Proxy;
use crate::transport::{ClientStream, MeteredStream, RecordingStream};

fn record_metered_client_io<S>(proxy: &Proxy, session_id: u64, client: &mut MeteredStream<S>)
where
    S: ClientStream,
{
    proxy.record_session_inbound_traffic(session_id, client.drain_traffic());
}

pub(super) async fn handle_vless_udp_session<S>(
    proxy: &Proxy,
    inbound_tag: &str,
    session: Session,
    relay: vless::VlessInboundUdpRelay<MeteredStream<RecordingStream<S>>>,
) -> Result<(), EngineError>
where
    S: ClientStream,
{
    let relay = relay.map_stream(|mut client| {
        proxy.record_session_inbound_traffic(session.id, client.drain_traffic());
        MeteredStream::new(client.into_unrecorded_inner())
    });
    run_protocol_stream_udp_relay(
        proxy,
        &session,
        relay,
        inbound_tag,
        "vless_udp",
        Some(record_metered_client_io::<S>),
    )
    .await
}
