use zero_core::ProtocolType;
use zero_engine::EngineError;

use crate::runtime::pipe::{KernelPipe, UdpPipe, UdpPipeInput};
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
    let udp_session = socks5::Socks5Inbound.udp_session();
    let Some((request, protocol_overhead_len)) = udp_session
        .decode_dispatch_parts_or_resolve_local_dns(packet, proxy.resolver.as_ref())
        .await?
    else {
        return Ok(());
    };

    // Generic dispatch.
    let session_id = UdpPipe::new(proxy, dispatch)
        .dispatch(UdpPipeInput {
            target: request.target,
            port: request.port,
            payload: &request.payload,
            protocol: ProtocolType::Socks5,
            auth: None,
            client_session_id: request.client_session_id,
        })
        .await?;

    // Record protocol-specific overhead: TCP control traffic and
    // SOCKS5 framing bytes (payload is already tracked by dispatch).
    proxy.record_session_inbound_traffic(session_id, *pending_control_traffic);
    *pending_control_traffic = StreamTraffic::default();
    proxy.record_session_inbound_rx(session_id, protocol_overhead_len as u64);

    Ok(())
}
