pub(crate) struct InboundUdpResponse {
    target: zero_core::Address,
    port: u16,
    payload: Vec<u8>,
}

impl InboundUdpResponse {
    pub(crate) fn target(&self) -> &zero_core::Address {
        &self.target
    }

    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    pub(crate) fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub(crate) fn into_parts(self) -> (zero_core::Address, u16, Vec<u8>) {
        (self.target, self.port, self.payload)
    }
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
