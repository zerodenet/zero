use zero_engine::EngineError;

use super::protocol_glue;
use crate::inbound::udp_dispatch::dispatch_inbound_udp_packet;
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::Proxy;
use crate::transport::StreamTraffic;

/// Parse a SOCKS5 UDP packet, handle DNS interception, then dispatch
/// via the generic `UdpDispatch`.
pub(super) async fn dispatch_packet(
    proxy: &Proxy,
    packet: &[u8],
    dispatch: &mut UdpDispatch,
    pending_control_traffic: &mut StreamTraffic,
) -> Result<(), EngineError> {
    let Some(request) = protocol_glue::decode_dispatch(proxy, packet).await? else {
        return Ok(());
    };
    let protocol_overhead = request.protocol_overhead();
    let inbound_dispatch = request.into_inbound_dispatch();

    // Generic dispatch.
    let session_id = dispatch_inbound_udp_packet(proxy, dispatch, &inbound_dispatch, None).await?;

    // Record protocol-specific overhead: TCP control traffic and
    // SOCKS5 framing bytes (payload is already tracked by dispatch).
    proxy.record_session_inbound_traffic(session_id, *pending_control_traffic);
    *pending_control_traffic = StreamTraffic::default();
    protocol_overhead.record(session_id, |session_id, bytes| {
        proxy.record_session_inbound_rx(session_id, bytes);
    });

    Ok(())
}
