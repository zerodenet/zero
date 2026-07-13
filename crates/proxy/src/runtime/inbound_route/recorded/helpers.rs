use core::future::Future;

use zero_core::{InboundMuxServer, Session, StreamUdpResponder};
use zero_engine::EngineError;

use super::model::RecordedProtocolMuxRouteDefaults;
use crate::runtime::mux_session::{run_protocol_mux_session, MuxSessionLoop};
use crate::runtime::stream_udp::run_mapped_protocol_stream_udp_relay;
use crate::runtime::Proxy;
use crate::transport::{ClientStream, MeteredStream, RecordingStream, TcpRelayStream};

pub(crate) fn into_recorded_tcp_relay_stream<S>(
    metered: MeteredStream<RecordingStream<S>>,
) -> TcpRelayStream
where
    S: ClientStream + 'static,
{
    TcpRelayStream::new(metered.into_unrecorded_inner())
}

pub(crate) fn record_metered_inbound_traffic<S>(
    proxy: &Proxy,
    session_id: u64,
    client: &mut MeteredStream<S>,
) where
    S: ClientStream,
{
    proxy.record_session_inbound_traffic(session_id, client.drain_traffic());
}

pub(crate) async fn run_recorded_protocol_stream_udp_relay<S, R>(
    proxy: Proxy,
    session: Session,
    relay: R,
    inbound_tag: String,
    protocol: &'static str,
) -> Result<(), EngineError>
where
    S: ClientStream + 'static,
    R: zero_core::InboundStreamUdpRelay<Stream = MeteredStream<RecordingStream<S>>>,
    R::Responder: StreamUdpResponder<MeteredStream<S>>,
{
    let session_id = session.id;
    let record_proxy = proxy.clone();
    run_mapped_protocol_stream_udp_relay(
        &proxy,
        &session,
        relay,
        &inbound_tag,
        protocol,
        move |mut client| {
            record_proxy.record_session_inbound_traffic(session_id, client.drain_traffic());
            MeteredStream::new(client.into_unrecorded_inner())
        },
        Some(record_metered_inbound_traffic::<S>),
    )
    .await
}

pub(crate) async fn run_recorded_protocol_mux_session<S, M, FTcp, FTcpFut, FUdp, FUdpFut>(
    proxy: Proxy,
    mut reader: MeteredStream<RecordingStream<S>>,
    mux_server: M,
    inbound_tag: String,
    defaults: RecordedProtocolMuxRouteDefaults,
    spawn_tcp: FTcp,
    spawn_udp: FUdp,
) -> Result<(), EngineError>
where
    S: ClientStream + 'static,
    M: InboundMuxServer<MeteredStream<S>>,
    FTcp: FnMut(Proxy, Session, M::TcpRelay, String) -> FTcpFut + Send,
    FTcpFut: Future<Output = ()> + Send + 'static,
    FUdp: FnMut(Proxy, M::UdpRelay, String) -> FUdpFut + Send,
    FUdpFut: Future<Output = ()> + Send + 'static,
{
    record_metered_inbound_traffic(&proxy, 0, &mut reader);
    let client = MeteredStream::new(reader.into_unrecorded_inner());
    run_protocol_mux_session(
        &proxy,
        client,
        mux_server,
        MuxSessionLoop {
            inbound_tag: &inbound_tag,
            protocol: defaults.mux_protocol,
            panic_message: defaults.panic_message,
            abort_on_end: defaults.abort_on_end,
        },
        spawn_tcp,
        spawn_udp,
    )
    .await
}
