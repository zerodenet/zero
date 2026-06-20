use super::*;

// Direct inbound is always available (no feature gate).
#[derive(Debug)]
pub(crate) struct DirectAdapter;

#[async_trait]
impl ProtocolAdapter for DirectAdapter {
    fn name(&self) -> &'static str {
        "direct"
    }
    fn feature_name(&self) -> &'static str {
        "core"
    }
    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        matches!(c, InboundProtocolConfig::Direct { .. })
    }
    fn supports_outbound(&self, _: &OutboundProtocolConfig) -> bool {
        false
    }
    fn has_inbound(&self) -> bool {
        true
    }
    fn has_outbound(&self) -> bool {
        false
    }
    fn claims_outbound_leaf(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool {
        matches!(leaf, ResolvedLeafOutbound::Direct { .. })
    }
    async fn connect_tcp(
        &self,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let ResolvedLeafOutbound::Direct { tag } = leaf else {
            return Err(unreachable_leaf(self.name(), leaf));
        };
        match proxy
            .protocols
            .direct_connector()
            .connect(session, proxy.resolver.as_ref())
            .await
        {
            Ok(upstream) => Ok(EstablishedTcpOutbound::Direct {
                tag: (*tag).unwrap_or("direct").to_string(),
                upstream: upstream.into(),
            }),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_direct",
                error: error.into(),
                upstream_endpoint: None,
            }),
        }
    }
    async fn start_udp_flow(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let ResolvedLeafOutbound::Direct { tag } = leaf else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        let target_addr = proxy
            .protocols
            .direct_connector()
            .resolve_target_addr(session, proxy.resolver.as_ref())
            .await
            .map_err(|error| FlowFailure {
                stage: "resolve_udp_target",
                error: error.into(),
                upstream: None,
            })?;
        let sent = dispatch
            .direct_socket
            .send_to_addr(payload, target_addr)
            .await
            .map_err(|error| FlowFailure {
                stage: "udp_direct_send",
                error: error.into(),
                upstream: None,
            })?;
        Ok(FlowStartResult::Flow {
            outbound: UdpFlowOutbound::Direct {
                tag: (*tag).unwrap_or("direct").to_string(),
                target_addr,
            },
            tx_bytes: sent as u64,
        })
    }
    fn spawn_inbound(
        &self,
        proxy: &Proxy,
        inbound: InboundConfig,
        bound: BoundInbound,
        shutdown_rx: tokio::sync::watch::Receiver<bool>,
        listeners: &mut tokio::task::JoinSet<Result<(), EngineError>>,
    ) {
        let p = proxy.clone();
        listeners.spawn(async move {
            crate::inbound::run_direct_listener_with_bound(
                &p,
                inbound,
                bound.into_tcp(),
                shutdown_rx,
            )
            .await
        });
    }
}

impl ProtocolMetadata for DirectAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        protocol_descriptor("direct", "core")
    }
}
