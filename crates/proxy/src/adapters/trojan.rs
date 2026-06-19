use super::*;

#[cfg(feature = "trojan")]
#[derive(Debug)]
pub(crate) struct TrojanAdapter;

#[cfg(feature = "trojan")]
#[async_trait]
impl ProtocolAdapter for TrojanAdapter {
    fn name(&self) -> &'static str {
        "trojan"
    }
    fn feature_name(&self) -> &'static str {
        "trojan"
    }
    fn has_inbound(&self) -> bool {
        true
    }
    fn has_outbound(&self) -> bool {
        true
    }
    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        matches!(c, InboundProtocolConfig::Trojan { .. })
    }
    fn supports_outbound(&self, c: &OutboundProtocolConfig) -> bool {
        matches!(c, OutboundProtocolConfig::Trojan { .. })
    }
    fn claims_outbound_leaf(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool {
        matches!(leaf, ResolvedLeafOutbound::Trojan { .. })
    }
    async fn connect_tcp(
        &self,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let ResolvedLeafOutbound::Trojan {
            tag,
            server,
            port,
            password,
            sni,
            insecure,
            client_fingerprint,
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf));
        };
        match crate::outbound::trojan::connect_tcp(
            proxy,
            session,
            server,
            *port,
            password,
            *sni,
            *insecure,
            *client_fingerprint,
        )
        .await
        {
            Ok(upstream) => Ok(EstablishedTcpOutbound::Trojan {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                upstream,
            }),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_upstream_trojan",
                error,
                upstream_endpoint: Some(((*server).to_string(), *port)),
            }),
        }
    }
    async fn apply_relay_hop(
        &self,
        proxy: &Proxy,
        stream: crate::transport::TcpRelayStream,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<crate::transport::TcpRelayStream, EngineError> {
        let ResolvedLeafOutbound::Trojan { password, .. } = leaf else {
            return Err(unreachable_leaf(self.name(), leaf).error);
        };
        crate::outbound::trojan::apply_tcp_hop(proxy, stream, session, password).await
    }
    async fn start_udp_flow(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let ResolvedLeafOutbound::Trojan {
            tag,
            server,
            port,
            password,
            sni,
            insecure,
            client_fingerprint,
        } = leaf
        else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        use crate::runtime::orchestration::OutboundEndpoint;
        use crate::runtime::udp_dispatch::{TrojanUdpPeer, UdpFlowContext, UdpPacketRef};
        let sent = dispatch
            .trojan_manager
            .send(
                UdpFlowContext {
                    chain_tasks: &mut dispatch.chain_tasks,
                    session_id: session.id,
                },
                proxy,
                session,
                TrojanUdpPeer {
                    endpoint: OutboundEndpoint {
                        server,
                        port: *port,
                    },
                    password,
                    sni: *sni,
                    insecure: *insecure,
                    client_fingerprint: *client_fingerprint,
                    relay_chain: false,
                },
                UdpPacketRef {
                    target: &session.target,
                    port: session.port,
                    payload,
                },
            )
            .await
            .map_err(|f: FlowFailure| FlowFailure {
                stage: f.stage,
                error: f.error,
                upstream: f.upstream,
            })?;
        Ok(FlowStartResult::Flow {
            outbound: UdpFlowOutbound::Trojan {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                password: (*password).to_string(),
                sni: (*sni).map(|s| s.to_string()),
                insecure: *insecure,
                client_fingerprint: (*client_fingerprint).map(|s| s.to_string()),
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
            p.run_trojan_listener_with_bound(inbound, bound.into_tcp(), shutdown_rx)
                .await
        });
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
        let ResolvedLeafOutbound::Trojan {
            tag,
            server,
            port,
            password,
            sni,
            insecure,
            client_fingerprint,
        } = leaf
        else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        use crate::runtime::orchestration::OutboundEndpoint;
        use crate::runtime::udp_dispatch::{TrojanUdpPeer, UdpFlowContext, UdpPacketRef};
        let sent = dispatch
            .trojan_manager
            .send_relay(
                UdpFlowContext {
                    chain_tasks: &mut dispatch.chain_tasks,
                    session_id: session.id,
                },
                carrier.stream,
                None,
                proxy,
                session,
                TrojanUdpPeer {
                    endpoint: OutboundEndpoint {
                        server,
                        port: *port,
                    },
                    password,
                    sni: *sni,
                    insecure: *insecure,
                    client_fingerprint: *client_fingerprint,
                    relay_chain: true,
                },
                UdpPacketRef {
                    target: &session.target,
                    port: session.port,
                    payload,
                },
            )
            .await?;
        Ok(FlowStartResult::Flow {
            outbound: UdpFlowOutbound::Trojan {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                password: (*password).to_string(),
                sni: (*sni).map(|s| s.to_string()),
                insecure: *insecure,
                client_fingerprint: (*client_fingerprint).map(|s| s.to_string()),
                relay_chain: true,
            },
            tx_bytes: sent as u64,
        })
    }
}

#[cfg(feature = "trojan")]
impl ProtocolMetadata for TrojanAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        ::trojan::TrojanProtocol.descriptor()
    }
}
