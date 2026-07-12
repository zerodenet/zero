use zero_core::SessionAuth;

pub(crate) struct DatagramUdpRelayRequest<'a, S, R> {
    pub(crate) source: S,
    pub(crate) responder: R,
    pub(crate) inbound_tag: &'a str,
    pub(crate) poll_upstream: bool,
    pub(crate) auth: Option<SessionAuth>,
}
