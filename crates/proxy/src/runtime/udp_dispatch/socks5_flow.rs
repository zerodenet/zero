use super::*;

impl UdpDispatch {
    /// Send via SOCKS5 upstream association, establishing one if needed.
    pub(crate) async fn send_socks5(
        &mut self,
        request: crate::protocol_runtime::socks5_udp::Socks5UdpSend<'_>,
    ) -> Result<usize, EngineError> {
        self.socks5.send(request, &self.inbound_tag).await
    }
}
