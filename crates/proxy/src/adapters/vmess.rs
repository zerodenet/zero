use super::*;

#[cfg(feature = "vmess")]
#[derive(Debug)]
pub(crate) struct VmessAdapter;

#[cfg(feature = "vmess")]
#[async_trait]
impl ProtocolAdapter for VmessAdapter {
    fn name(&self) -> &'static str {
        "vmess"
    }
    fn feature_name(&self) -> &'static str {
        "vmess"
    }
    fn has_inbound(&self) -> bool {
        true
    }
    fn has_outbound(&self) -> bool {
        true
    }
    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        matches!(c, InboundProtocolConfig::Vmess { .. })
    }
    fn supports_outbound(&self, c: &OutboundProtocolConfig) -> bool {
        matches!(c, OutboundProtocolConfig::Vmess { .. })
    }
    fn claims_outbound_leaf(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool {
        matches!(leaf, ResolvedLeafOutbound::Vmess { .. })
    }
    async fn connect_tcp(
        &self,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let ResolvedLeafOutbound::Vmess {
            tag,
            server,
            port,
            id,
            cipher,
            mux_concurrency,
            mux_idle_timeout_secs,
            tls,
            ws,
            grpc,
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf));
        };
        match crate::outbound::vmess::connect_tcp(
            proxy,
            session,
            server,
            *port,
            id,
            cipher,
            *mux_concurrency,
            *mux_idle_timeout_secs,
            *tls,
            *ws,
            *grpc,
        )
        .await
        {
            Ok(upstream) => Ok(EstablishedTcpOutbound::Vmess {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                upstream,
            }),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_upstream_vmess",
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
        let ResolvedLeafOutbound::Vmess { id, cipher, .. } = leaf else {
            return Err(unreachable_leaf(self.name(), leaf).error);
        };
        crate::outbound::vmess::apply_tcp_hop(stream, session, id, cipher).await
    }
    async fn start_udp_flow(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        use crate::runtime::vmess_udp::VmessUdpTransport;

        let ResolvedLeafOutbound::Vmess {
            tag,
            server,
            port,
            id,
            cipher,
            mux_concurrency,
            mux_idle_timeout_secs: _,
            tls,
            ws,
            grpc,
        } = leaf
        else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        let transport = VmessUdpTransport {
            tls: *tls,
            ws: *ws,
            grpc: *grpc,
        };
        let session_id = session.id;
        let tag_owned = (*tag).to_string();
        dispatch
            .vmess_manager
            .get_or_create_upstream(
                &mut dispatch.chain_tasks,
                proxy,
                session,
                session.target.clone(),
                session.port,
                (*server).to_string(),
                *port,
                (*id).to_string(),
                (*cipher).to_string(),
                payload.to_vec(),
                Some(&transport),
                *mux_concurrency,
            )
            .await
            .map_err(|error| FlowFailure {
                stage: "udp_vmess_upstream",
                error,
                upstream: Some(((*server).to_string(), *port)),
            })?;

        Ok(FlowStartResult::VmessFlow {
            session_id,
            tag: tag_owned,
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
            p.run_vmess_listener_with_bound(inbound, bound.into_tcp(), shutdown_rx)
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
        use crate::runtime::vmess_udp::{
            build_vmess_udp_transport_over_stream, establish_vmess_udp_upstream_over_stream,
            VmessUdpTransport,
        };

        let ResolvedLeafOutbound::Vmess {
            tag,
            server,
            port,
            id,
            cipher,
            tls,
            ws,
            grpc,
            ..
        } = leaf
        else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        let session_id = session.id;
        let tag_owned = (*tag).to_string();
        let key = (session.target.clone(), session.port);
        let transport = VmessUdpTransport {
            tls: *tls,
            ws: *ws,
            grpc: *grpc,
        };
        let stream = build_vmess_udp_transport_over_stream(
            carrier.stream,
            Some(&transport),
            proxy.config.source_dir(),
            server,
            *port,
        )
        .await
        .map_err(|error| FlowFailure {
            stage: "udp_vmess_relay_final_transport",
            error,
            upstream: Some(((*server).to_string(), *port)),
        })?;
        let (upstream, recv_tx) =
            establish_vmess_udp_upstream_over_stream(proxy, session, id, cipher, payload, stream)
                .await
                .map_err(|error| FlowFailure {
                    stage: "udp_vmess_relay_chain",
                    error,
                    upstream: None,
                })?;
        dispatch
            .vmess_manager
            .insert_upstream(key, upstream, recv_tx);
        dispatch.vmess_manager.spawn_bridge(
            &mut dispatch.chain_tasks,
            session.target.clone(),
            session.port,
            session_id,
        );

        Ok(FlowStartResult::VmessFlow {
            session_id,
            tag: tag_owned,
        })
    }
}

#[cfg(feature = "vmess")]
impl ProtocolMetadata for VmessAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        ::vmess::VmessProtocol.descriptor()
    }
}
