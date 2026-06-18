//! Concrete `ProtocolAdapter` implementations for each compiled-in protocol.

use std::sync::Arc;

use async_trait::async_trait;

use zero_config::{InboundConfig, InboundProtocolConfig, OutboundProtocolConfig};
use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata, TransportKind};

use crate::protocol_capability::protocol_descriptor;
use crate::runtime::udp_associate::sessions::UdpFlowOutbound;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::Proxy;
use crate::transport::{EstablishedTcpOutbound, QuicInbound, TcpOutboundFailure};

use super::protocol_adapter::{BoundInbound, ProtocolAdapter};

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
            .send_socks5(
                proxy, tag, server, *port, *username, *password, session, payload,
            )
            .await
            .map_err(|error| FlowFailure {
                stage: "udp_upstream_send",
                error,
                upstream: Some(((*server).to_string(), *port)),
            })?;
        Ok(FlowStartResult::Flow {
            outbound: UdpFlowOutbound::Socks5 {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                username: (*username).map(|u| u.to_string()),
                password: (*password).map(|p| p.to_string()),
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
            p.run_socks5_listener_with_bound(inbound, bound.into_tcp(), shutdown_rx)
                .await
        });
    }
}

#[cfg(feature = "socks5")]
impl ProtocolMetadata for Socks5Adapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        socks5::Socks5Protocol.descriptor()
    }
}

#[cfg(feature = "http_connect")]
#[derive(Debug)]
pub(crate) struct HttpConnectAdapter;

#[cfg(feature = "http_connect")]
impl ProtocolAdapter for HttpConnectAdapter {
    fn name(&self) -> &'static str {
        "http_connect"
    }

    fn feature_name(&self) -> &'static str {
        "http_connect"
    }

    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        matches!(c, InboundProtocolConfig::HttpConnect)
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
            p.run_http_connect_listener_with_bound(inbound, bound.into_tcp(), shutdown_rx)
                .await
        });
    }
}

#[cfg(feature = "http_connect")]
impl ProtocolMetadata for HttpConnectAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        http_connect::HttpConnectProtocol.descriptor()
    }
}

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
    fn inbound_transport_kind(&self, c: &InboundProtocolConfig) -> TransportKind {
        match c {
            InboundProtocolConfig::Vless { quic: Some(_), .. } => TransportKind::Quic,
            _ => TransportKind::Tcp,
        }
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
        use crate::runtime::vless_udp::VlessUdpTransport;
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
            let idle_timeout = 300u64;
            match proxy
                .mux_pool
                .open_udp_stream(
                    proxy,
                    server,
                    *port,
                    &vless::parse_uuid(id).map_err(|e| FlowFailure {
                        stage: "udp_vless_mux_parse_uuid",
                        error: zero_core::Error::Protocol(&*Box::leak(
                            format!("invalid VLESS UUID: {e}").into_boxed_str(),
                        ))
                        .into(),
                        upstream: Some(((*server).to_string(), *port)),
                    })?,
                    *tls,
                    *reality,
                    max_concurrency,
                    idle_timeout,
                )
                .await
            {
                Ok((_mux_sid, up_tx, _down_rx)) => {
                    let packet = <vless::VlessOutbound as UdpPacketFraming<
                        vless::VlessUdpPacketTarget,
                    >>::encode_udp_packet(
                        &proxy.protocols.vless_outbound,
                        &vless::VlessUdpPacketTarget {
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
                Err(_) => {}
            }
        }

        let transport = VlessUdpTransport {
            tls: *tls,
            reality: *reality,
            ws: *ws,
            grpc: *grpc,
            h2: *h2,
            http_upgrade: *http_upgrade,
            split_http: *split_http,
            quic: *quic,
        };
        dispatch
            .vless_manager
            .get_or_create_upstream(
                &mut dispatch.chain_tasks,
                proxy,
                session,
                session.target.clone(),
                session.port,
                (*server).to_string(),
                *port,
                (*id).to_string(),
                payload.to_vec(),
                Some(&transport),
            )
            .await
            .map_err(|error| FlowFailure {
                stage: "udp_vless_upstream",
                error,
                upstream: Some(((*server).to_string(), *port)),
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
            p.run_vless_listener_with_bound(inbound, bound, shutdown_rx)
                .await
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
        use crate::runtime::vless_udp::establish_vless_udp_upstream_over_stream;

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
            server,
            port,
            id,
            split_http,
            ..
        } = &final_hop
        else {
            return Err(unreachable_udp_leaf(self.name(), &final_hop));
        };
        let split_http_cfg = split_http
            .as_ref()
            .expect("udp_relay_needs_two_streams checked split_http is Some");

        let stream = crate::transport::build_vless_split_http_over_relay(
            post_carrier.stream,
            get_carrier.stream,
            split_http_cfg,
        )
        .await
        .map_err(|error| FlowFailure {
            stage: "udp_relay_final_transport",
            error,
            upstream: Some(((*server).to_string(), *port)),
        })?;

        let session_id = session.id;
        let key = (session.target.clone(), session.port);
        let (upstream, recv_tx) =
            establish_vless_udp_upstream_over_stream(proxy, session, id, payload, stream)
                .await
                .map_err(|error| FlowFailure {
                    stage: "udp_vless_relay_chain",
                    error,
                    upstream: None,
                })?;
        dispatch
            .vless_manager
            .insert_upstream(key, upstream, recv_tx);
        dispatch.vless_manager.spawn_bridge(
            &mut dispatch.chain_tasks,
            session.target.clone(),
            session.port,
            session_id,
        );

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
        use crate::runtime::vless_udp::establish_vless_udp_upstream_over_stream;

        let ResolvedLeafOutbound::Vless {
            tag,
            server,
            port,
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

        let session_id = session.id;
        let tag_owned = (*tag).to_string();
        let key = (session.target.clone(), session.port);
        let stream = crate::transport::build_vless_outbound_transport_over_stream(
            carrier,
            *tls,
            *reality,
            *ws,
            *grpc,
            *h2,
            *http_upgrade,
            *split_http,
            proxy.config.source_dir(),
        )
        .await
        .map_err(|error| FlowFailure {
            stage: "udp_relay_final_transport",
            error,
            upstream: Some(((*server).to_string(), *port)),
        })?;
        let (upstream, recv_tx) =
            establish_vless_udp_upstream_over_stream(proxy, session, id, payload, stream)
                .await
                .map_err(|error| FlowFailure {
                    stage: "udp_vless_relay_chain",
                    error,
                    upstream: None,
                })?;
        dispatch
            .vless_manager
            .insert_upstream(key, upstream, recv_tx);
        dispatch.vless_manager.spawn_bridge(
            &mut dispatch.chain_tasks,
            session.target.clone(),
            session.port,
            session_id,
        );

        Ok(FlowStartResult::VlessFlow {
            session_id,
            tag: tag_owned,
        })
    }
}

#[cfg(feature = "vless")]
impl ProtocolMetadata for VlessAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        vless::VlessProtocol.descriptor()
    }
}

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
    fn inbound_transport_kind(&self, _c: &InboundProtocolConfig) -> TransportKind {
        TransportKind::Quic
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
        use crate::runtime::orchestration::OutboundEndpoint;
        use crate::runtime::udp_dispatch::{H2UdpPeer, UdpFlowContext, UdpPacketRef};
        let sent = dispatch
            .h2_manager
            .send(
                UdpFlowContext {
                    chain_tasks: &mut dispatch.chain_tasks,
                    session_id: session.id,
                },
                H2UdpPeer {
                    endpoint: OutboundEndpoint {
                        server,
                        port: *port,
                    },
                    password,
                    client_fingerprint: *client_fingerprint,
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
            outbound: UdpFlowOutbound::Hysteria2 {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                password: (*password).to_string(),
                client_fingerprint: (*client_fingerprint).map(|s| s.to_string()),
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
            p.run_hysteria2_listener_with_bound(inbound, bound, shutdown_rx)
                .await
        });
    }
}

#[cfg(feature = "hysteria2")]
impl ProtocolMetadata for Hysteria2Adapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        hysteria2::Hysteria2Protocol.descriptor()
    }
}

#[cfg(feature = "shadowsocks")]
#[derive(Debug)]
pub(crate) struct ShadowsocksAdapter;

#[cfg(feature = "shadowsocks")]
#[async_trait]
impl ProtocolAdapter for ShadowsocksAdapter {
    fn name(&self) -> &'static str {
        "shadowsocks"
    }
    fn feature_name(&self) -> &'static str {
        "shadowsocks"
    }
    fn has_inbound(&self) -> bool {
        true
    }
    fn has_outbound(&self) -> bool {
        true
    }
    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        matches!(c, InboundProtocolConfig::Shadowsocks { .. })
    }
    fn supports_outbound(&self, c: &OutboundProtocolConfig) -> bool {
        matches!(c, OutboundProtocolConfig::Shadowsocks { .. })
    }
    fn claims_outbound_leaf(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool {
        matches!(leaf, ResolvedLeafOutbound::Shadowsocks { .. })
    }
    async fn connect_tcp(
        &self,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let ResolvedLeafOutbound::Shadowsocks {
            tag,
            server,
            port,
            password,
            cipher,
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf));
        };
        match crate::outbound::shadowsocks::connect_tcp(
            proxy, session, server, *port, password, cipher,
        )
        .await
        {
            Ok(upstream) => Ok(EstablishedTcpOutbound::Shadowsocks {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                upstream,
            }),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_upstream_shadowsocks",
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
        let ResolvedLeafOutbound::Shadowsocks {
            password, cipher, ..
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf).error);
        };
        crate::outbound::shadowsocks::apply_tcp_hop(stream, session, password, cipher).await
    }
    async fn start_udp_flow(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let ResolvedLeafOutbound::Shadowsocks {
            tag,
            server,
            port,
            password,
            cipher,
            ..
        } = leaf
        else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        use crate::runtime::orchestration::OutboundEndpoint;
        use crate::runtime::udp_dispatch::{SsUdpPeer, UdpFlowContext, UdpPacketRef};
        let sent = dispatch
            .ss_manager
            .send(
                UdpFlowContext {
                    chain_tasks: &mut dispatch.chain_tasks,
                    session_id: session.id,
                },
                proxy,
                SsUdpPeer {
                    endpoint: OutboundEndpoint {
                        server,
                        port: *port,
                    },
                    password,
                    cipher,
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
            outbound: UdpFlowOutbound::Shadowsocks {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                password: (*password).to_string(),
                cipher: (*cipher).to_string(),
                packet_path_carrier: None,
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
            p.run_shadowsocks_listener_with_bound(inbound, bound.into_tcp(), shutdown_rx)
                .await
        });
    }
}

#[cfg(feature = "shadowsocks")]
impl ProtocolMetadata for ShadowsocksAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        shadowsocks::ShadowsocksProtocol.descriptor()
    }
}

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
        trojan::TrojanProtocol.descriptor()
    }
}

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
        vmess::VmessProtocol.descriptor()
    }
}

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
        use crate::runtime::orchestration::OutboundEndpoint;
        use crate::runtime::udp_dispatch::{MieruUdpPeer, UdpFlowContext, UdpPacketRef};
        let sent = dispatch
            .mieru_manager
            .send(
                UdpFlowContext {
                    chain_tasks: &mut dispatch.chain_tasks,
                    session_id: session.id,
                },
                proxy,
                session,
                MieruUdpPeer {
                    endpoint: OutboundEndpoint {
                        server,
                        port: *port,
                    },
                    username,
                    password,
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
            p.run_mieru_listener_with_bound(inbound, bound.into_tcp(), shutdown_rx)
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
        use crate::runtime::orchestration::OutboundEndpoint;
        use crate::runtime::udp_dispatch::{MieruUdpPeer, UdpFlowContext, UdpPacketRef};
        let sent = dispatch
            .mieru_manager
            .send_relay(
                UdpFlowContext {
                    chain_tasks: &mut dispatch.chain_tasks,
                    session_id: session.id,
                },
                carrier.stream,
                MieruUdpPeer {
                    endpoint: OutboundEndpoint {
                        server,
                        port: *port,
                    },
                    username,
                    password,
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
        mieru::MieruProtocol.descriptor()
    }
}

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
            .direct_outbound
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
            .direct_outbound
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
            p.run_direct_listener_with_bound(inbound, bound.into_tcp(), shutdown_rx)
                .await
        });
    }
}

impl ProtocolMetadata for DirectAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        protocol_descriptor("direct", "core")
    }
}

/// Build and return the protocol registry with all compiled-in adapters.
pub(crate) fn build_registry() -> super::protocol_adapter::ProtocolRegistry {
    let mut r = super::protocol_adapter::ProtocolRegistry::default();

    #[cfg(feature = "socks5")]
    r.register(Arc::new(Socks5Adapter));
    #[cfg(feature = "http_connect")]
    r.register(Arc::new(HttpConnectAdapter));
    #[cfg(feature = "vless")]
    r.register(Arc::new(VlessAdapter));
    #[cfg(feature = "hysteria2")]
    r.register(Arc::new(Hysteria2Adapter));
    #[cfg(feature = "shadowsocks")]
    r.register(Arc::new(ShadowsocksAdapter));
    #[cfg(feature = "trojan")]
    r.register(Arc::new(TrojanAdapter));
    #[cfg(feature = "vmess")]
    r.register(Arc::new(VmessAdapter));
    #[cfg(feature = "mieru")]
    r.register(Arc::new(MieruAdapter));
    // Always available.
    r.register(Arc::new(DirectAdapter));

    r
}

/// Build a `TcpOutboundFailure` for the impossible case where an adapter's
/// `connect_tcp` receives a leaf variant it did not claim.
///
/// `claims_outbound_leaf` guarantees the variant matches before the runtime
/// dispatches `connect_tcp`, so this only fires on a programming error.
fn unreachable_leaf(adapter: &'static str, _leaf: &ResolvedLeafOutbound<'_>) -> TcpOutboundFailure {
    TcpOutboundFailure {
        stage: "outbound_leaf_mismatch",
        error: EngineError::Io(std::io::Error::other(format!(
            "{adapter} adapter received a non-matching outbound leaf"
        ))),
        upstream_endpoint: None,
    }
}

/// Same as [`unreachable_leaf`] but for the UDP `start_udp_flow` path.
fn unreachable_udp_leaf(adapter: &'static str, _leaf: &ResolvedLeafOutbound<'_>) -> FlowFailure {
    FlowFailure {
        stage: "udp_leaf_mismatch",
        error: EngineError::Io(std::io::Error::other(format!(
            "{adapter} adapter received a non-matching UDP leaf"
        ))),
        upstream: None,
    }
}
