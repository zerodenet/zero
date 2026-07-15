use std::path::Path;

use zero_engine::EngineError;
use zero_transport::outbound_leaf::{
    PreparedTransportBridgeLeaf, ProtocolTcpTransportBridgeOps, ProtocolTcpTransportLeafMetadata,
    ProtocolTcpTransportOpenResult, ProtocolTransportLeaf,
};

use super::super::{ClaimedTcpOutboundLeaf, OutboundLeafRuntime};
use crate::runtime::tcp_dispatch::operation::{
    PreparedTcpConnectOperation, PreparedTcpRelayOperation,
};
use crate::transport::TcpOutboundFailure;

pub(crate) fn claim_transport_bridge_tcp_leaf<'a, TBridge, TLeaf, F, E>(
    bridge: TBridge,
    upstream: Option<(&'a str, u16)>,
    runtime: OutboundLeafRuntime,
    prepare_leaf: F,
) -> Box<dyn ClaimedTcpOutboundLeaf<'a> + 'a>
where
    TBridge: Send + Sync + Clone + 'a + ProtocolTcpTransportBridgeOps<TLeaf>,
    TLeaf: ProtocolTransportLeaf + ProtocolTcpTransportLeafMetadata + Send + Sync + 'a,
    TBridge::Opened: ProtocolTcpTransportOpenResult,
    F: Fn(Option<&Path>) -> Result<TLeaf, E> + Send + Sync + 'a,
    E: std::fmt::Display,
{
    Box::new(ClaimedTransportBridgeTcpLeaf {
        bridge,
        upstream,
        runtime,
        prepare_leaf,
    })
}

struct ClaimedTransportBridgeTcpLeaf<'a, TBridge, F> {
    bridge: TBridge,
    upstream: Option<(&'a str, u16)>,
    runtime: OutboundLeafRuntime,
    prepare_leaf: F,
}

impl<'a, TBridge, TLeaf, F, E> ClaimedTcpOutboundLeaf<'a>
    for ClaimedTransportBridgeTcpLeaf<'a, TBridge, F>
where
    TBridge: Send + Sync + Clone + 'a + ProtocolTcpTransportBridgeOps<TLeaf>,
    TLeaf: ProtocolTransportLeaf + ProtocolTcpTransportLeafMetadata + Send + Sync + 'a,
    TBridge::Opened: ProtocolTcpTransportOpenResult,
    F: Fn(Option<&Path>) -> Result<TLeaf, E> + Send + Sync + 'a,
    E: std::fmt::Display,
{
    fn runtime(&self) -> OutboundLeafRuntime {
        self.runtime.clone()
    }

    fn prepare_tcp_connect(
        &self,
        source_dir: Option<&Path>,
    ) -> Result<Box<dyn PreparedTcpConnectOperation + 'a>, TcpOutboundFailure> {
        let prepared = (self.prepare_leaf)(source_dir)
            .map(PreparedTransportBridgeLeaf::new)
            .map_err(|error| {
                transport_bridge_connect_claim_prepare_failure::<TLeaf, _>(self.upstream, error)
            })?;
        Ok(prepare_transport_bridge_tcp_connect(&self.bridge, prepared))
    }

    fn prepare_tcp_relay_hop(
        &self,
        source_dir: Option<&Path>,
    ) -> Result<Box<dyn PreparedTcpRelayOperation + 'a>, EngineError> {
        let prepared = (self.prepare_leaf)(source_dir)
            .map(PreparedTransportBridgeLeaf::new)
            .map_err(transport_bridge_relay_claim_prepare_error::<TLeaf, _>)?;
        Ok(prepare_transport_bridge_tcp_relay(&self.bridge, prepared))
    }
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
pub(crate) fn prepare_transport_bridge_tcp_connect<'a, TBridge, TLeaf>(
    bridge: &TBridge,
    prepared: PreparedTransportBridgeLeaf<TLeaf>,
) -> Box<dyn crate::runtime::tcp_dispatch::operation::PreparedTcpConnectOperation + 'a>
where
    TBridge: Send + Sync + Clone + 'a + ProtocolTcpTransportBridgeOps<TLeaf>,
    TLeaf: ProtocolTransportLeaf + ProtocolTcpTransportLeafMetadata + Send + Sync + 'a,
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

fn transport_bridge_connect_claim_prepare_failure<TLeaf, E>(
    upstream: Option<(&str, u16)>,
    error: E,
) -> TcpOutboundFailure
where
    TLeaf: ProtocolTcpTransportLeafMetadata,
    E: std::fmt::Display,
{
    TcpOutboundFailure {
        stage: TLeaf::TCP_CONNECT_STAGE,
        error: invalid_input(TLeaf::TCP_INVALID_CONNECT_CONFIG, error),
        upstream_endpoint: upstream.map(|(server, port)| (server.to_owned(), port)),
    }
}

fn transport_bridge_relay_claim_prepare_error<TLeaf, E>(error: E) -> EngineError
where
    TLeaf: ProtocolTcpTransportLeafMetadata,
    E: std::fmt::Display,
{
    invalid_input(TLeaf::TCP_INVALID_RELAY_CONFIG, error)
}

fn invalid_input(stage: &'static str, error: impl std::fmt::Display) -> EngineError {
    EngineError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!("{stage}: {error}"),
    ))
}
