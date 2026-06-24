use zero_engine::EngineError;

use crate::protocol_runtime::udp::ProtocolUdpFlowSnapshot;
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::Proxy;
use zero_core::Session;

pub(crate) struct Socks5RelaySend<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) tag: &'a str,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) protocol: &'a ProtocolUdpFlowSnapshot,
    pub(crate) session: &'a Session,
    pub(crate) payload: &'a [u8],
}

impl UdpDispatch {
    /// Send via SOCKS5 upstream association, establishing one if needed.
    pub(crate) async fn send_socks5(
        &mut self,
        request: Socks5RelaySend<'_>,
    ) -> Result<usize, EngineError> {
        let auth = request.protocol.socks5_relay_auth().ok_or_else(|| {
            EngineError::Io(std::io::Error::other(
                "relay protocol snapshot is not a SOCKS5 UDP relay",
            ))
        })?;
        let packet = crate::protocol_runtime::socks5_udp::Socks5UdpPacketSend {
            proxy: request.proxy,
            tag: request.tag,
            server: request.server,
            port: request.port,
            username: auth.username,
            password: auth.password,
            session: request.session,
            payload: request.payload,
        };
        self.socks5.send_packet(packet, &self.inbound_tag).await
    }
}
