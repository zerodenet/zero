use zero_core::{InboundStreamUdpRelay, Session, StreamUdpResponder};
use zero_engine::EngineError;

use crate::runtime::stream_udp::run_mapped_protocol_stream_udp_relay;
use crate::runtime::Proxy;

/// Run a Mieru UDP relay through the generic UDP pipe.
pub(super) async fn run_mieru_udp_relay<R>(
    proxy: &Proxy,
    relay: R,
    session: &Session,
    inbound_tag: &str,
) -> Result<(), EngineError>
where
    R: InboundStreamUdpRelay,
    R::Stream: Send,
    R::Responder: StreamUdpResponder<R::Stream>,
{
    run_mapped_protocol_stream_udp_relay(
        proxy,
        session,
        relay,
        inbound_tag,
        "mieru_udp",
        core::convert::identity,
        None,
    )
    .await
}
