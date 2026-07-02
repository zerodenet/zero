pub(crate) struct VmessInboundRequest {
    pub(crate) inbound: zero_config::InboundConfig,
    pub(crate) profile: vmess::VmessInboundProfile,
    pub(crate) tls_acceptor: crate::transport::TlsAcceptor,
    pub(crate) ws: Option<Box<zero_config::WebSocketConfig>>,
    pub(crate) grpc: Option<Box<zero_config::GrpcConfig>>,
}
