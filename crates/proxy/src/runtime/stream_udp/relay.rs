use tracing::info;
use zero_core::{InboundStreamUdpRelay, Session, SessionAuth, StreamUdpResponder};
use zero_engine::EngineError;

use super::handler::StreamPacketSessionUdpHandler;
use crate::runtime::packet_session_udp::{
    run_packet_session_udp_relay, PacketSessionUdpFailurePolicy, PacketSessionUdpRelayRequest,
};
use crate::runtime::udp_ingress::UdpIngressRuntime;

pub(crate) struct StreamUdpRelayRequest<'a, S, R> {
    pub(crate) client: S,
    pub(crate) responder: R,
    pub(crate) session: &'a Session,
    pub(crate) inbound_tag: &'a str,
    pub(crate) protocol: &'static str,
    pub(crate) auth: Option<SessionAuth>,
    pub(crate) record_client_io: Option<fn(&UdpIngressRuntime, u64, &mut S)>,
}

pub(crate) async fn run_mapped_protocol_stream_udp_relay<R, S, F>(
    runtime: UdpIngressRuntime,
    session: &Session,
    relay: R,
    inbound_tag: &str,
    protocol: &'static str,
    map_client: F,
    record_client_io: Option<fn(&UdpIngressRuntime, u64, &mut S)>,
) -> Result<(), EngineError>
where
    R: InboundStreamUdpRelay,
    R::Responder: StreamUdpResponder<S>,
    S: Send,
    F: FnOnce(R::Stream) -> S,
{
    let (client, responder, auth) = relay.into_stream_udp_parts();
    run_stream_udp_relay(
        runtime,
        StreamUdpRelayRequest {
            client: map_client(client),
            responder,
            session,
            inbound_tag,
            protocol,
            auth,
            record_client_io,
        },
    )
    .await
}

async fn run_stream_udp_relay<S, R>(
    runtime: UdpIngressRuntime,
    request: StreamUdpRelayRequest<'_, S, R>,
) -> Result<(), EngineError>
where
    S: Send,
    R: StreamUdpResponder<S>,
{
    let StreamUdpRelayRequest {
        client,
        responder,
        session,
        inbound_tag,
        protocol,
        auth,
        record_client_io,
    } = request;

    info!(
        inbound_tag = inbound_tag,
        protocol = protocol,
        "stream udp session started"
    );

    let handler = StreamPacketSessionUdpHandler {
        runtime: runtime.clone(),
        client,
        responder,
        stream_session_id: session.id,
        record_client_io,
    };

    run_packet_session_udp_relay(
        runtime,
        PacketSessionUdpRelayRequest {
            handler,
            inbound_tag,
            protocol,
            auth,
            failure_policy: PacketSessionUdpFailurePolicy::ReturnError,
        },
    )
    .await?;

    info!(
        inbound_tag = inbound_tag,
        protocol = protocol,
        "stream udp session ended"
    );

    Ok(())
}
