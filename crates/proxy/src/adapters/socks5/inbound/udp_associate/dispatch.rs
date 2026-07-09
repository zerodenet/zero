use zero_engine::EngineError;
use zero_traits::DnsResolver;

use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_inbound_dispatch::dispatch_inbound_udp_packet;
use crate::runtime::Proxy;
use crate::transport::StreamTraffic;

struct Socks5InboundUdpDispatchBridge<'a> {
    proxy: &'a Proxy,
    dispatch: &'a mut UdpDispatch,
    pending_control_traffic: &'a mut StreamTraffic,
}

impl socks5::udp::Socks5InboundUdpDispatchActionDispatcher for Socks5InboundUdpDispatchBridge<'_> {
    type Error = EngineError;

    async fn dispatch_local_dns(&mut self, domain: &str) -> Result<(), Self::Error> {
        let _ = self.proxy.resolver.resolve(domain).await;
        Ok(())
    }

    async fn dispatch_inbound_packet(
        &mut self,
        request: socks5::udp::Socks5InboundUdpDispatchView,
    ) -> Result<(), Self::Error> {
        let protocol_overhead = request.protocol_overhead();
        let inbound_dispatch = request.into_inbound_dispatch();

        let session_id =
            dispatch_inbound_udp_packet(self.proxy, self.dispatch, &inbound_dispatch, None).await?;

        self.proxy
            .record_session_inbound_traffic(session_id, *self.pending_control_traffic);
        *self.pending_control_traffic = StreamTraffic::default();
        protocol_overhead.record(session_id, |session_id, bytes| {
            self.proxy.record_session_inbound_rx(session_id, bytes);
        });

        Ok(())
    }
}

/// Parse a SOCKS5 UDP packet, handle DNS interception, then dispatch
/// via the generic `UdpDispatch`.
pub(super) async fn dispatch_packet(
    proxy: &Proxy,
    association: &socks5::udp::Socks5InboundUdpAssociationSession,
    packet: &[u8],
    dispatch: &mut UdpDispatch,
    pending_control_traffic: &mut StreamTraffic,
) -> Result<(), EngineError> {
    let mut bridge = Socks5InboundUdpDispatchBridge {
        proxy,
        dispatch,
        pending_control_traffic,
    };
    association
        .dispatch_client_packet(packet, &mut bridge)
        .await
}
