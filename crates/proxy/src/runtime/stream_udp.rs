use tracing::info;
use zero_core::{InboundStreamUdpRelay, Session, SessionAuth, StreamUdpResponder};
use zero_engine::EngineError;

use crate::runtime::packet_session_udp::{
    run_packet_session_udp_relay, PacketSessionUdpFailurePolicy, PacketSessionUdpHandler,
    PacketSessionUdpReadFailure, PacketSessionUdpReadFailureAction, PacketSessionUdpReadResult,
    PacketSessionUdpRelayRequest,
};
use crate::runtime::Proxy;

pub(crate) struct StreamUdpRelayRequest<'a, S, R> {
    pub(crate) client: S,
    pub(crate) responder: R,
    pub(crate) session: &'a Session,
    pub(crate) inbound_tag: &'a str,
    pub(crate) protocol: &'static str,
    pub(crate) auth: Option<SessionAuth>,
    pub(crate) record_client_io: Option<fn(&Proxy, u64, &mut S)>,
}

pub(crate) async fn run_mapped_protocol_stream_udp_relay<R, S, F>(
    proxy: &Proxy,
    session: &Session,
    relay: R,
    inbound_tag: &str,
    protocol: &'static str,
    map_client: F,
    record_client_io: Option<fn(&Proxy, u64, &mut S)>,
) -> Result<(), EngineError>
where
    R: InboundStreamUdpRelay,
    R::Responder: StreamUdpResponder<S>,
    S: Send,
    F: FnOnce(R::Stream) -> S,
{
    let (client, responder, auth) = relay.into_stream_udp_parts();
    run_stream_udp_relay(
        proxy,
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

struct StreamPacketSessionUdpHandler<'a, S, R> {
    proxy: &'a Proxy,
    client: S,
    responder: R,
    stream_session_id: u64,
    record_client_io: Option<fn(&Proxy, u64, &mut S)>,
}

impl<S, R> PacketSessionUdpHandler for StreamPacketSessionUdpHandler<'_, S, R>
where
    S: Send,
    R: StreamUdpResponder<S>,
{
    async fn read_inbound_dispatch(
        &mut self,
    ) -> Result<PacketSessionUdpReadResult, PacketSessionUdpReadFailure> {
        match self.responder.read_inbound_dispatch(&mut self.client).await {
            Ok(Some(inbound_dispatch)) => {
                record_stream_udp_client_io(
                    self.proxy,
                    self.record_client_io,
                    self.stream_session_id,
                    &mut self.client,
                );
                Ok(PacketSessionUdpReadResult::Dispatch(inbound_dispatch))
            }
            Ok(None) => Ok(PacketSessionUdpReadResult::End),
            Err(error) => Err(PacketSessionUdpReadFailure {
                error,
                action: PacketSessionUdpReadFailureAction::End,
            }),
        }
    }

    async fn write_response_for_target(
        &mut self,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, zero_core::Error> {
        let written = self
            .responder
            .write_response_for_target(&mut self.client, target, port, payload)
            .await?;
        record_stream_udp_client_io(
            self.proxy,
            self.record_client_io,
            self.stream_session_id,
            &mut self.client,
        );
        Ok(written)
    }
}

pub(crate) async fn run_stream_udp_relay<S, R>(
    proxy: &Proxy,
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
        proxy,
        client,
        responder,
        stream_session_id: session.id,
        record_client_io,
    };

    run_packet_session_udp_relay(
        proxy,
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

fn record_stream_udp_client_io<S>(
    proxy: &Proxy,
    record_client_io: Option<fn(&Proxy, u64, &mut S)>,
    session_id: u64,
    client: &mut S,
) {
    if let Some(record_client_io) = record_client_io {
        record_client_io(proxy, session_id, client);
    }
}
