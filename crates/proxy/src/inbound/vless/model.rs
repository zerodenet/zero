use tokio::sync::mpsc;
use zero_core::SessionAuth;

use super::ConfiguredVlessUser;

pub(crate) struct VlessInboundRequest {
    pub(crate) inbound: zero_config::InboundConfig,
    pub(crate) users: std::sync::Arc<[ConfiguredVlessUser]>,
}

pub(crate) struct VlessMuxUdpStreamTask<'a> {
    pub(crate) mux_session_id: u16,
    pub(crate) up_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    pub(crate) down_tx: mpsc::UnboundedSender<(u16, Vec<u8>)>,
    pub(crate) inbound_tag: &'a str,
    pub(crate) auth: Option<&'a SessionAuth>,
}
