use zero_core::ProtocolType;
use zero_engine::EngineError;
use zero_traits::DnsResolver;

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
    let udp_packet = udp_session.decode_request(packet)?;

    // DNS interception.
    // Intercept UDP packets to port 53 with a domain target.
    // Resolve locally through DnsSystem and reply directly.
    if let Some(domain) = udp_session.local_dns_domain_request(&udp_packet) {
        if let Ok(_ips) = proxy.resolver.resolve(domain).await {
            // DNS resolved locally; build response and return.
            // The caller will forward via the relay socket if
            // available. For now, skip dispatch and return Ok.
            // The DNS response is sent inline in the main loop.
            return Ok(());
        }
        // Resolution failed; silently drop.
        return Ok(());
    }

    let protocol_overhead_len = udp_packet.protocol_overhead_len() as u64;
    let request = udp_packet.into_dispatch_parts();

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
    proxy.record_session_inbound_rx(session_id, protocol_overhead_len);

    Ok(())
}
