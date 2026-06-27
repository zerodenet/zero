pub(crate) struct InboundUdpResponse {
    pub(crate) target: zero_core::Address,
    pub(crate) port: u16,
    pub(crate) payload: Vec<u8>,
}

pub(crate) fn decode_socks5_upstream_response(packet: &[u8]) -> Option<InboundUdpResponse> {
    socks5::decode_udp_associate_response(packet)
        .ok()
        .map(|response| InboundUdpResponse {
            target: response.target,
            port: response.port,
            payload: response.payload,
        })
}
