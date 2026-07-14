use zero_transport::outbound_leaf::{
    PreparedTransportBridgeLeaf, ProtocolTcpTransportBridgeMetadata, ProtocolTcpTransportBridgeOps,
    ProtocolTcpTransportOpenResult, ProtocolTransportLeaf,
};

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) fn prepare_transport_bridge_tcp_connect<'a, TBridge, TLeaf>(
    bridge: &TBridge,
    prepared: PreparedTransportBridgeLeaf<TLeaf>,
) -> Box<dyn crate::runtime::tcp_dispatch::operation::PreparedTcpConnectOperation + 'a>
where
    TBridge: Send
        + Sync
        + Clone
        + 'a
        + ProtocolTcpTransportBridgeMetadata
        + ProtocolTcpTransportBridgeOps<TLeaf>,
    TLeaf: ProtocolTransportLeaf + Send + Sync + 'a,
    TBridge::Opened: ProtocolTcpTransportOpenResult,
{
    Box::new(
        crate::runtime::tcp_dispatch::operation::TransportBridgeTcpConnectOperation {
            bridge: bridge.clone(),
            prepared,
        },
    )
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) fn prepare_transport_bridge_tcp_relay<'a, TBridge, TLeaf>(
    bridge: &TBridge,
    prepared: PreparedTransportBridgeLeaf<TLeaf>,
) -> Box<dyn crate::runtime::tcp_dispatch::operation::PreparedTcpRelayOperation + 'a>
where
    TBridge: Send + Sync + Clone + 'a + ProtocolTcpTransportBridgeOps<TLeaf>,
    TLeaf: Send + Sync + 'a,
{
    Box::new(
        crate::runtime::tcp_dispatch::operation::TransportBridgeTcpRelayOperation {
            bridge: bridge.clone(),
            prepared,
        },
    )
}
