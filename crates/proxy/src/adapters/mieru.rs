use super::*;

#[cfg(feature = "mieru")]
#[derive(Debug)]
pub(crate) struct MieruAdapter;

#[cfg(feature = "mieru")]
#[async_trait]
impl ProtocolAdapter for MieruAdapter {
    fn name(&self) -> &'static str {
        "mieru"
    }
    fn feature_name(&self) -> &'static str {
        "mieru"
    }
    fn has_inbound(&self) -> bool {
        true
    }
    fn has_outbound(&self) -> bool {
        true
    }
    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        matches!(c, InboundProtocolConfig::Mieru { .. })
    }
    fn supports_outbound(&self, c: &OutboundProtocolConfig) -> bool {
        matches!(c, OutboundProtocolConfig::Mieru { .. })
    }
    fn claims_outbound_leaf(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool {
        matches!(leaf, ResolvedLeafOutbound::Mieru { .. })
    }
    async fn connect_tcp(
        &self,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let ResolvedLeafOutbound::Mieru {
            tag,
            server,
            port,
            username,
            password,
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf));
        };
        match crate::outbound::mieru::connect_tcp(proxy, session, server, *port, username, password)
            .await
        {
            Ok(upstream) => Ok(EstablishedTcpOutbound::Mieru {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                upstream,
            }),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_upstream_mieru",
                error,
                upstream_endpoint: Some(((*server).to_string(), *port)),
            }),
        }
    }
    async fn apply_relay_hop(
        &self,
        _proxy: &Proxy,
        stream: crate::transport::TcpRelayStream,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<crate::transport::TcpRelayStream, EngineError> {
        let ResolvedLeafOutbound::Mieru {
            username, password, ..
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf).error);
        };
        crate::outbound::mieru::apply_tcp_hop(stream, session, username, password).await
    }
    async fn start_udp_flow(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let ResolvedLeafOutbound::Mieru {
            tag,
            server,
            port,
            username,
            password,
        } = leaf
        else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        let sent = dispatch
            .start_mieru_udp_flow(
                proxy, session, server, *port, username, password, false, payload,
            )
            .await
            .map_err(|f: FlowFailure| FlowFailure {
                stage: f.stage,
                error: f.error,
                upstream: f.upstream,
            })?;
        Ok(FlowStartResult::Flow {
            outbound: UdpFlowOutbound::Mieru {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                username: (*username).to_string(),
                password: (*password).to_string(),
                relay_chain: false,
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
            crate::inbound::run_mieru_listener_with_bound(
                &p,
                inbound,
                bound.into_tcp(),
                shutdown_rx,
            )
            .await
        });
    }
    async fn start_udp_relay_final_hop(
        &self,
        dispatch: &mut UdpDispatch,
        _proxy: &Proxy,
        session: &Session,
        carrier: crate::transport::RelayCarrier,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let ResolvedLeafOutbound::Mieru {
            tag,
            server,
            port,
            username,
            password,
        } = leaf
        else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        let sent = dispatch
            .start_mieru_udp_relay_flow(
                session, carrier, server, *port, username, password, payload,
            )
            .await?;
        Ok(FlowStartResult::Flow {
            outbound: UdpFlowOutbound::Mieru {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                username: (*username).to_string(),
                password: (*password).to_string(),
                relay_chain: true,
            },
            tx_bytes: sent as u64,
        })
    }
}

#[cfg(feature = "mieru")]
impl ProtocolMetadata for MieruAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        ::mieru::MieruProtocol.descriptor()
    }
}
