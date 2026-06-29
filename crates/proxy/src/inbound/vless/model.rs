use zero_core::SessionAuth;

pub(crate) struct VlessInboundRequest {
    pub(crate) inbound: zero_config::InboundConfig,
    pub(crate) profile: vless::VlessInboundProfile,
    pub(crate) reality: Option<vless::VlessRealityServerProfile>,
    pub(crate) tls: Option<Box<zero_config::TlsConfig>>,
    pub(crate) ws: Option<Box<zero_config::WebSocketConfig>>,
    pub(crate) grpc: Option<Box<zero_config::GrpcConfig>>,
    pub(crate) h2: Option<Box<zero_config::H2Config>>,
    pub(crate) http_upgrade: Option<Box<zero_config::HttpUpgradeConfig>>,
    pub(crate) split_http: Option<Box<zero_config::SplitHttpConfig>>,
    pub(crate) fallback: Option<Box<zero_config::FallbackConfig>>,
}

pub(crate) struct VlessMuxUdpStreamTask<'a> {
    pub(crate) mux_session_id: u16,
    pub(crate) up_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
    pub(crate) writer: vless::VlessInboundMuxWriter,
    pub(crate) inbound_tag: &'a str,
    pub(crate) auth: Option<&'a SessionAuth>,
}
