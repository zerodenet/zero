use zero_core::SessionAuth;

pub(crate) struct VlessInboundRequest {
    pub(crate) inbound: zero_config::InboundConfig,
    pub(crate) users: std::sync::Arc<[vless::VlessConfiguredUser]>,
    pub(crate) reality: Option<vless::VlessRealityServerProfile>,
}

pub(crate) struct VlessMuxUdpStreamTask<'a> {
    pub(crate) mux_session_id: u16,
    pub(crate) up_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
    pub(crate) writer: vless::VlessInboundMuxWriter,
    pub(crate) inbound_tag: &'a str,
    pub(crate) auth: Option<&'a SessionAuth>,
}
