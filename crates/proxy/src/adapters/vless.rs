use super::*;

#[cfg(feature = "vless")]
mod inbound;
#[cfg(feature = "vless")]
mod tcp;
#[cfg(feature = "vless")]
mod udp;

#[cfg(feature = "vless")]
#[derive(Debug)]
pub(crate) struct VlessAdapter;

#[cfg(feature = "vless")]
#[async_trait]
impl ProtocolAdapter for VlessAdapter {
    fn name(&self) -> &'static str {
        "vless"
    }
    fn feature_name(&self) -> &'static str {
        "vless"
    }
    fn has_inbound(&self) -> bool {
        true
    }
    fn has_outbound(&self) -> bool {
        true
    }
    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        matches!(c, InboundProtocolConfig::Vless { .. })
    }
    fn supports_outbound(&self, c: &OutboundProtocolConfig) -> bool {
        matches!(c, OutboundProtocolConfig::Vless { .. })
    }
    fn claims_outbound_leaf(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool {
        matches!(leaf, ResolvedLeafOutbound::Vless { .. })
    }
    fn outbound_leaf_runtime<'a>(
        &self,
        leaf: &ResolvedLeafOutbound<'a>,
    ) -> Option<OutboundLeafRuntime<'a>> {
        proxy_leaf_runtime(leaf, TcpPathCategory::Tunnel)
    }
    async fn bind_inbound(
        &self,
        inbound: &InboundConfig,
        source_dir: Option<&std::path::Path>,
    ) -> Result<BoundInbound, EngineError> {
        self.bind_inbound_impl(inbound, source_dir).await
    }
    async fn connect_tcp(
        &self,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        self.connect_tcp_impl(proxy, session, leaf).await
    }
    async fn apply_relay_hop(
        &self,
        proxy: &Proxy,
        stream: crate::transport::TcpRelayStream,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<crate::transport::TcpRelayStream, EngineError> {
        self.apply_relay_hop_impl(proxy, stream, session, leaf)
            .await
    }
    async fn start_udp_flow(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        self.start_udp_flow_impl(dispatch, proxy, session, leaf, payload)
            .await
    }
    fn spawn_inbound(
        &self,
        proxy: &Proxy,
        inbound: InboundConfig,
        bound: BoundInbound,
        shutdown_rx: tokio::sync::watch::Receiver<bool>,
        listeners: &mut tokio::task::JoinSet<Result<(), EngineError>>,
    ) {
        self.spawn_inbound_impl(proxy, inbound, bound, shutdown_rx, listeners);
    }
    fn udp_relay_needs_two_streams(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool {
        self.udp_relay_needs_two_streams_impl(leaf)
    }
    async fn start_udp_relay_two_stream(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        chain: Vec<ResolvedLeafOutbound<'_>>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        self.start_udp_relay_two_stream_impl(dispatch, proxy, session, chain, payload)
            .await
    }
    async fn start_udp_relay_final_hop(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        carrier: crate::transport::RelayCarrier,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        self.start_udp_relay_final_hop_impl(dispatch, proxy, session, carrier, leaf, payload)
            .await
    }
}

#[cfg(feature = "vless")]
impl ProtocolMetadata for VlessAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        ::vless::VlessProtocol.descriptor()
    }
}
