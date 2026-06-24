use zero_engine::EngineError;

use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_flow::outbound::Socks5UdpRelay;
use crate::runtime::Proxy;
use zero_core::Session;

impl UdpDispatch {
    /// Send via SOCKS5 upstream association, establishing one if needed.
    pub(crate) async fn send_socks5(
        &mut self,
        proxy: &Proxy,
        relay: Socks5UdpRelay<'_>,
        session: &Session,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        let request = crate::protocol_runtime::socks5_udp::Socks5UdpPacketSend {
            proxy,
            tag: relay.tag,
            server: relay.server,
            port: relay.port,
            username: relay.username,
            password: relay.password,
            session,
            payload,
        };
        self.socks5.send_packet(request, &self.inbound_tag).await
    }
}
