pub(crate) struct VlessInboundRequest {
    pub(crate) inbound: zero_config::InboundConfig,
    pub(crate) profile: vless::VlessInboundProfile,
    pub(crate) reality: Option<vless::VlessRealityServerProfile>,
    pub(crate) tls_acceptor: Option<crate::transport::TlsAcceptor>,
    pub(crate) ws: Option<Box<zero_config::WebSocketConfig>>,
    pub(crate) grpc: Option<Box<zero_config::GrpcConfig>>,
    pub(crate) h2: Option<Box<zero_config::H2Config>>,
    pub(crate) http_upgrade: Option<Box<zero_config::HttpUpgradeConfig>>,
    pub(crate) split_http: Option<Box<zero_config::SplitHttpConfig>>,
    pub(crate) fallback: Option<Box<zero_config::FallbackConfig>>,
}
