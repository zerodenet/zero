use super::*;

#[cfg(feature = "socks5")]
#[derive(Debug)]
pub(crate) struct Socks5Adapter;

#[cfg(feature = "socks5")]
#[async_trait]
impl ProtocolAdapter for Socks5Adapter {
    fn name(&self) -> &'static str {
        "socks5"
    }

    fn feature_name(&self) -> &'static str {
        "socks5"
    }

    fn has_inbound(&self) -> bool {
        true
    }

    fn has_outbound(&self) -> bool {
        true
    }

    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        matches!(c, InboundProtocolConfig::Socks5 { .. })
    }

    fn supports_outbound(&self, c: &OutboundProtocolConfig) -> bool {
        matches!(c, OutboundProtocolConfig::Socks5 { .. })
    }

    fn claims_outbound_leaf(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool {
        matches!(leaf, ResolvedLeafOutbound::Socks5 { .. })
    }

    #[cfg(feature = "shadowsocks")]
    fn udp_packet_path_carrier_descriptor(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::runtime::udp_dispatch::PacketPathCarrierDescriptor> {
        let ResolvedLeafOutbound::Socks5 {
            tag,
            server,
            port,
            username,
            password,
        } = leaf
        else {
            return None;
        };
        let auth = match (username, password) {
            (Some(user), Some(_)) => format!("|auth:{user}"),
            _ => String::new(),
        };
        Some(crate::runtime::udp_dispatch::PacketPathCarrierDescriptor {
            cache_key: format!("socks5|{tag}|{server}:{port}{auth}"),
            server: (*server).to_string(),
            port: *port,
        })
    }

    #[cfg(feature = "shadowsocks")]
    fn udp_packet_path_carrier_snapshot(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::runtime::udp_associate::sessions::UdpPacketPathCarrier> {
        use crate::runtime::udp_associate::sessions::UdpPacketPathCarrier;

        let ResolvedLeafOutbound::Socks5 {
            tag,
            server,
            port,
            username,
            password,
        } = leaf
        else {
            return None;
        };
        let auth = match (username, password) {
            (Some(user), Some(_)) => format!("|auth:{user}"),
            _ => String::new(),
        };
        Some(UdpPacketPathCarrier::Socks5 {
            cache_key: format!("socks5|{tag}|{server}:{port}{auth}"),
            tag: (*tag).to_string(),
            server: (*server).to_string(),
            port: *port,
            username: (*username).map(|value| value.to_string()),
            password: (*password).map(|value| value.to_string()),
        })
    }

    #[cfg(feature = "shadowsocks")]
    async fn build_udp_packet_path(
        &self,
        proxy: &Proxy,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<Arc<dyn crate::runtime::udp_dispatch::PacketPathCarrier>, EngineError> {
        let ResolvedLeafOutbound::Socks5 {
            tag,
            server,
            port,
            username,
            password,
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf).error);
        };
        crate::runtime::udp_dispatch::build_socks5_packet_path(
            proxy,
            tag,
            server,
            *port,
            username.zip(*password),
        )
        .await
    }

    async fn connect_tcp(
        &self,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let ResolvedLeafOutbound::Socks5 {
            tag,
            server,
            port,
            username,
            password,
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf));
        };
        match crate::outbound::socks5::connect_tcp(
            proxy,
            session,
            server,
            *port,
            username.zip(*password),
        )
        .await
        {
            Ok(upstream) => Ok(EstablishedTcpOutbound::Socks5 {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                upstream,
            }),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_upstream_socks5",
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
        let ResolvedLeafOutbound::Socks5 {
            username, password, ..
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf).error);
        };
        crate::outbound::socks5::apply_tcp_hop(proxy, stream, session, username.zip(*password))
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
        let ResolvedLeafOutbound::Socks5 {
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
            .send_socks5(crate::runtime::udp_dispatch::Socks5UdpSend {
                proxy,
                tag,
                server,
                port: *port,
                username: *username,
                password: *password,
                session,
                payload,
            })
            .await
            .map_err(|error| FlowFailure {
                stage: "udp_upstream_send",
                error,
                upstream: Some(((*server).to_string(), *port)),
            })?;
        Ok(FlowStartResult::Flow {
            outbound: Box::new(UdpFlowOutbound::Socks5 {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                username: (*username).map(|u| u.to_string()),
                password: (*password).map(|p| p.to_string()),
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
            crate::inbound::run_socks5_listener_with_bound(
                &p,
                inbound,
                bound.into_tcp(),
                shutdown_rx,
            )
            .await
        });
    }
}

#[cfg(feature = "socks5")]
impl ProtocolMetadata for Socks5Adapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        ::socks5::Socks5Protocol.descriptor()
    }
}
