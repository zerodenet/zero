use super::*;

#[cfg(feature = "hysteria2")]
#[derive(Debug)]
pub(crate) struct Hysteria2Adapter;

#[cfg(feature = "hysteria2")]
#[async_trait]
impl ProtocolAdapter for Hysteria2Adapter {
    fn name(&self) -> &'static str {
        "hysteria2"
    }
    fn feature_name(&self) -> &'static str {
        "hysteria2"
    }
    fn has_inbound(&self) -> bool {
        true
    }
    fn has_outbound(&self) -> bool {
        true
    }
    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        matches!(c, InboundProtocolConfig::Hysteria2 { .. })
    }
    fn supports_outbound(&self, c: &OutboundProtocolConfig) -> bool {
        matches!(c, OutboundProtocolConfig::Hysteria2 { .. })
    }
    fn claims_outbound_leaf(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool {
        matches!(leaf, ResolvedLeafOutbound::Hysteria2 { .. })
    }

    fn udp_packet_path_carrier_descriptor(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::runtime::udp_dispatch::PacketPathCarrierDescriptor> {
        let ResolvedLeafOutbound::Hysteria2 {
            tag,
            server,
            port,
            password,
            client_fingerprint,
            ..
        } = leaf
        else {
            return None;
        };
        let fingerprint = client_fingerprint
            .map(|value| format!("|fp:{value}"))
            .unwrap_or_default();
        Some(crate::runtime::udp_dispatch::PacketPathCarrierDescriptor {
            cache_key: format!("hysteria2|{tag}|{server}:{port}|{password}{fingerprint}"),
            server: (*server).to_string(),
            port: *port,
        })
    }

    fn udp_packet_path_carrier_snapshot(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::protocol_runtime::udp::UdpPacketPathCarrier> {
        use crate::protocol_runtime::udp::UdpPacketPathCarrier;

        let ResolvedLeafOutbound::Hysteria2 {
            tag,
            server,
            port,
            password,
            client_fingerprint,
            ..
        } = leaf
        else {
            return None;
        };
        let fingerprint = client_fingerprint
            .map(|value| format!("|fp:{value}"))
            .unwrap_or_default();
        Some(UdpPacketPathCarrier::Hysteria2 {
            cache_key: format!("hysteria2|{tag}|{server}:{port}|{password}{fingerprint}"),
            tag: (*tag).to_string(),
            server: (*server).to_string(),
            port: *port,
            password: (*password).to_string(),
            client_fingerprint: (*client_fingerprint).map(|value| value.to_string()),
        })
    }

    #[cfg(feature = "shadowsocks")]
    async fn build_udp_packet_path(
        &self,
        _proxy: &Proxy,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<Arc<dyn crate::runtime::udp_dispatch::PacketPathCarrier>, EngineError> {
        let ResolvedLeafOutbound::Hysteria2 {
            server,
            port,
            password,
            client_fingerprint,
            ..
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf).error);
        };
        crate::runtime::udp_dispatch::build_hysteria2_packet_path(
            server,
            *port,
            password,
            *client_fingerprint,
        )
        .await
    }

    async fn bind_inbound(
        &self,
        inbound: &InboundConfig,
        source_dir: Option<&std::path::Path>,
    ) -> Result<BoundInbound, EngineError> {
        let listen = format!("{}:{}", inbound.listen.address, inbound.listen.port);
        if let InboundProtocolConfig::Hysteria2 {
            cert_path,
            key_path,
            ..
        } = &inbound.protocol
        {
            let cert = cert_path
                .clone()
                .unwrap_or_else(|| "certs/fullchain.pem".to_string());
            let key = key_path
                .clone()
                .unwrap_or_else(|| "certs/privkey.pem".to_string());
            let endpoint = QuicInbound::bind(&listen, &cert, &key, source_dir).await?;
            Ok(BoundInbound::Quic(endpoint))
        } else {
            unreachable!("hysteria2 adapter only handles Hysteria2 config")
        }
    }
    async fn connect_tcp(
        &self,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let ResolvedLeafOutbound::Hysteria2 {
            tag,
            server,
            port,
            password,
            insecure: _,
            client_fingerprint,
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf));
        };
        match crate::outbound::hysteria2::connect_tcp(
            proxy,
            session,
            server,
            *port,
            password,
            *client_fingerprint,
        )
        .await
        {
            Ok(upstream) => Ok(EstablishedTcpOutbound::Hysteria2 {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                upstream,
            }),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_upstream_hysteria2",
                error,
                upstream_endpoint: Some(((*server).to_string(), *port)),
            }),
        }
    }
    async fn start_udp_flow(
        &self,
        dispatch: &mut UdpDispatch,
        _proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let ResolvedLeafOutbound::Hysteria2 {
            tag,
            server,
            port,
            password,
            client_fingerprint,
            ..
        } = leaf
        else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        let sent = dispatch
            .start_hysteria2_udp_flow(
                session,
                server,
                *port,
                password,
                *client_fingerprint,
                payload,
            )
            .await
            .map_err(|f: FlowFailure| FlowFailure {
                stage: f.stage,
                error: f.error,
                upstream: f.upstream,
            })?;
        Ok(FlowStartResult::Flow {
            outbound: Box::new(UdpFlowOutbound::Hysteria2 {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                password: (*password).to_string(),
                client_fingerprint: (*client_fingerprint).map(|s| s.to_string()),
            }),
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
            crate::inbound::run_hysteria2_listener_with_bound(&p, inbound, bound, shutdown_rx).await
        });
    }
}

#[cfg(feature = "hysteria2")]
impl ProtocolMetadata for Hysteria2Adapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        ::hysteria2::Hysteria2Protocol.descriptor()
    }
}
