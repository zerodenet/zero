use crate::runtime::Proxy;

pub(crate) struct StreamRouteBridge<P, FMapTcp, FRunUdp> {
    pub(crate) proxy: Proxy,
    pub(crate) inbound_tag: String,
    pub(crate) source_addr: Option<std::net::SocketAddr>,
    pub(crate) protocol: P,
    pub(crate) map_tcp_stream: FMapTcp,
    pub(crate) run_udp: FRunUdp,
}
