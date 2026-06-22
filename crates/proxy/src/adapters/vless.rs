use super::*;

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
    async fn bind_inbound(
        &self,
        inbound: &InboundConfig,
        source_dir: Option<&std::path::Path>,
    ) -> Result<BoundInbound, EngineError> {
        let listen = format!("{}:{}", inbound.listen.address, inbound.listen.port);
        if let InboundProtocolConfig::Vless {
            quic: Some(ref quic),
            ..
        } = inbound.protocol
        {
            if let (Some(cert), Some(key)) = (&quic.cert_path, &quic.key_path) {
                let endpoint = QuicInbound::bind(&listen, cert, key, source_dir).await?;
                return Ok(BoundInbound::Quic(endpoint));
            }
        }
        let tcp = zero_platform_tokio::TokioListener::bind(&listen)
            .await
            .map_err(EngineError::Io)?;
        Ok(BoundInbound::Tcp(tcp))
    }
    async fn connect_tcp(
        &self,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let ResolvedLeafOutbound::Vless {
            tag,
            server,
            port,
            id,
            flow,
            mux_concurrency,
            mux_idle_timeout_secs,
            tls,
            reality,
            ws,
            grpc,
            h2,
            http_upgrade,
            split_http,
            quic,
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf));
        };
        match crate::outbound::vless::connect_tcp(
            proxy,
            session,
            server,
            *port,
            id,
            *flow,
            *mux_concurrency,
            *mux_idle_timeout_secs,
            *tls,
            *reality,
            *ws,
            *grpc,
            *h2,
            *http_upgrade,
            *quic,
            *split_http,
        )
        .await
        {
            Ok(upstream) => Ok(EstablishedTcpOutbound::Vless {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                upstream,
            }),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_upstream_vless",
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
        let ResolvedLeafOutbound::Vless { id, flow, .. } = leaf else {
            return Err(unreachable_leaf(self.name(), leaf).error);
        };
        crate::outbound::vless::apply_tcp_hop(proxy, stream, session, id, *flow).await
    }
    async fn start_udp_flow(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        use zero_traits::UdpPacketFraming;

        let ResolvedLeafOutbound::Vless {
            tag,
            server,
            port,
            id,
            flow,
            tls,
            reality,
            ws,
            grpc,
            h2,
            http_upgrade,
            split_http,
            quic,
            ..
        } = leaf
        else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        let session_id = session.id;
        let tag_owned = (*tag).to_string();

        // MUX UDP fast path: when Vision flow is active, open a UDP sub-stream
        // through the shared MUX connection instead of dialing fresh.
        let mux_flow_enabled =
            *flow == Some("xtls-rprx-vision") || *flow == Some("xtls-rprx-vision-udp443");
        if mux_flow_enabled {
            let max_concurrency = 8u32;
            if let Ok((_mux_sid, up_tx, _down_rx)) = proxy
                .mux_pool
                .open_udp_stream(crate::runtime::mux_pool::VlessMuxOpenRequest {
                    proxy,
                    session: None,
                    server,
                    port: *port,
                    id: &::vless::parse_uuid(id).map_err(|e| FlowFailure {
                        stage: "udp_vless_mux_parse_uuid",
                        error: EngineError::Io(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            format!("invalid VLESS UUID: {e}"),
                        )),
                        upstream: Some(((*server).to_string(), *port)),
                    })?,
                    tls: *tls,
                    reality: *reality,
                    max_concurrency,
                })
                .await
            {
                let packet = <::vless::VlessOutbound as UdpPacketFraming<
                    ::vless::VlessUdpPacketTarget,
                >>::encode_udp_packet(
                    &proxy.protocols.vless_outbound_protocol(),
                    &::vless::VlessUdpPacketTarget {
                        address: &session.target,
                        port: session.port,
                        payload,
                    },
                )
                .map_err(|error| FlowFailure {
                    stage: "udp_vless_mux_encode",
                    error: error.into(),
                    upstream: Some(((*server).to_string(), *port)),
                })?;
                let _ = up_tx.send(packet);
                proxy.record_session_outbound_tx(session_id, payload.len() as u64);
                return Ok(FlowStartResult::VlessFlow {
                    session_id,
                    tag: tag_owned,
                });
            }
        }

        dispatch
            .start_vless_udp_flow(VlessUdpFlow {
                proxy,
                session,
                server,
                port: *port,
                id,
                flow: *flow,
                tls: *tls,
                reality: *reality,
                ws: *ws,
                grpc: *grpc,
                h2: *h2,
                http_upgrade: *http_upgrade,
                split_http: *split_http,
                quic: *quic,
                payload,
            })
            .await
            .map_err(|error| FlowFailure {
                stage: error.stage,
                error: error.error,
                upstream: error.upstream,
            })?;

        Ok(FlowStartResult::VlessFlow {
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
            crate::inbound::run_vless_listener_with_bound(&p, inbound, bound, shutdown_rx).await
        });
    }
    fn udp_relay_needs_two_streams(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool {
        matches!(
            leaf,
            ResolvedLeafOutbound::Vless {
                split_http: Some(cfg),
                ..
            } if !zero_transport::split_http::XhttpMode::parse(&cfg.mode).is_single_connection()
        )
    }
    async fn start_udp_relay_two_stream(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        chain: Vec<ResolvedLeafOutbound<'_>>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let chain_get = chain.clone();
        let (post_carrier, final_hop) =
            proxy
                .dispatch_tcp_relay_prefix(chain)
                .await
                .map_err(|f| FlowFailure {
                    stage: f.stage,
                    error: f.error,
                    upstream: f.upstream_endpoint,
                })?;
        let (get_carrier, _) = proxy
            .dispatch_tcp_relay_prefix(chain_get)
            .await
            .map_err(|f| FlowFailure {
                stage: f.stage,
                error: f.error,
                upstream: f.upstream_endpoint,
            })?;

        let ResolvedLeafOutbound::Vless {
            tag,
            server: _,
            port: _,
            id,
            split_http,
            ..
        } = &final_hop
        else {
            return Err(unreachable_udp_leaf(self.name(), &final_hop));
        };
        let session_id = session.id;
        let split_http_cfg = split_http
            .as_ref()
            .expect("udp_relay_needs_two_streams checked split_http is Some");
        dispatch
            .start_vless_udp_relay_two_stream(VlessUdpRelayTwoStream {
                proxy,
                session,
                post_carrier,
                get_carrier,
                id,
                split_http: split_http_cfg,
                payload,
            })
            .await?;

        Ok(FlowStartResult::VlessFlow {
            session_id,
            tag: (*tag).to_string(),
        })
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
        let ResolvedLeafOutbound::Vless {
            tag,
            server: _,
            port: _,
            id,
            tls,
            reality,
            ws,
            grpc,
            h2,
            http_upgrade,
            split_http,
            quic,
            ..
        } = leaf
        else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        let session_id = session.id;
        if quic.is_some() {
            return Err(FlowFailure {
                stage: "udp_relay_final_transport",
                error: zero_core::Error::Unsupported(
                    "VLESS QUIC final hop over TCP relay chain is not supported",
                )
                .into(),
                upstream: None,
            });
        }

        let tag_owned = (*tag).to_string();
        dispatch
            .start_vless_udp_relay_final_hop(VlessUdpRelayFinalHop {
                proxy,
                session,
                carrier,
                id,
                tls: *tls,
                reality: *reality,
                ws: *ws,
                grpc: *grpc,
                h2: *h2,
                http_upgrade: *http_upgrade,
                split_http: *split_http,
                payload,
            })
            .await?;

        Ok(FlowStartResult::VlessFlow {
            session_id,
            tag: tag_owned,
        })
    }
}

#[cfg(feature = "vless")]
impl ProtocolMetadata for VlessAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        ::vless::VlessProtocol.descriptor()
    }
}
