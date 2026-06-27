pub(crate) struct InboundUdpResponse {
    pub(crate) target: zero_core::Address,
    pub(crate) port: u16,
    pub(crate) payload: Vec<u8>,
}

pub(crate) fn decode_socks5_upstream_response(packet: &[u8]) -> Option<InboundUdpResponse> {
    socks5::Socks5InboundUdpSession::new()
        .decode_response(packet)
        .ok()
        .map(|response| {
            let (target, port, payload) = response.into_parts();
            InboundUdpResponse {
                target,
                port,
                payload,
            }
        })
}
