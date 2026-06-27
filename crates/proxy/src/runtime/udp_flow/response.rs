use zero_core::Address;

pub(crate) struct UpstreamUdpResponse {
    target: Address,
    port: u16,
    payload: Vec<u8>,
}

impl UpstreamUdpResponse {
    pub(crate) fn new(target: Address, port: u16, payload: Vec<u8>) -> Self {
        Self {
            target,
            port,
            payload,
        }
    }

    pub(crate) fn target(&self) -> &Address {
        &self.target
    }

    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    pub(crate) fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub(crate) fn into_parts(self) -> (Address, u16, Vec<u8>) {
        (self.target, self.port, self.payload)
    }
}
