use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::adapters::common::unreachable_udp_leaf;
use crate::adapters::vmess::VmessAdapter;
use crate::protocol_adapter::ProtocolSupportCapability;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
use crate::runtime::Proxy;
use manager::{
    model::{VmessUdpRelayFlowStart, VmessUdpStartFlow},
    VmessUdpOutboundManager,
};

mod manager;

fn parse_vmess_udp_identity(
    id: &str,
    cipher: &str,
    stage: &'static str,
    upstream: Option<(&str, u16)>,
) -> Result<vmess::VmessUdpIdentity, FlowFailure> {
    vmess::parse_udp_identity(id, cipher).map_err(|error| FlowFailure {
        stage,
        error: zero_engine::EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid VMess UDP identity: {error}"),
        )),
        upstream: upstream.map(|(server, port)| (server.to_string(), port)),
    })
}

impl VmessAdapter {
    pub(super) async fn start_udp_flow_impl(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
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
        let tag_owned = (*tag).to_string();
        let identity = parse_vmess_udp_identity(
            id,
            cipher,
            "udp_vmess_parse_identity",
            Some((server, *port)),
        )?;
        let transport = crate::transport::VmessTransportOptions {
            tls: *tls,
            ws: *ws,
            grpc: *grpc,
            source_dir: proxy.config.source_dir(),
        };
        let mut manager = VmessUdpOutboundManager::new();
        manager
            .start_flow(
                dispatch.protocol_udp_chain_tasks(),
                VmessUdpStartFlow {
                    proxy,
                    mux_pool: &self.mux_pool,
                    session,
                    server,
                    port: *port,
                    identity,
                    cipher_name: cipher,
                    mux_concurrency: *mux_concurrency,
                    transport,
                    payload,
                },
            )
            .await
            .map_err(|error| FlowFailure {
                stage: "udp_vmess_upstream",
                error,
                upstream: Some((server.to_string(), *port)),
            })?;
        dispatch.register_cached_protocol_flow_sender(Box::new(manager));

        Ok(FlowStartResult::Flow {
            outbound: Box::new(UdpFlowOutbound::Cached {
                tag: tag_owned,
                server: (*server).to_string(),
                port: *port,
            }),
            tx_bytes: 0,
        })
    }

    pub(super) async fn start_udp_relay_final_hop_impl(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        carrier: crate::transport::RelayCarrier,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
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
        let tag_owned = (*tag).to_string();
        let identity = parse_vmess_udp_identity(
            id,
            cipher,
            "udp_vmess_relay_final_hop_parse_identity",
            Some((server, *port)),
        )?;
        let transport = crate::transport::VmessTransportOptions {
            tls: *tls,
            ws: *ws,
            grpc: *grpc,
            source_dir: proxy.config.source_dir(),
        };
        let mut manager = VmessUdpOutboundManager::new();
        manager
            .start_relay_flow(
                dispatch.protocol_udp_chain_tasks(),
                VmessUdpRelayFlowStart {
                    proxy,
                    session,
                    carrier,
                    identity,
                    transport,
                    payload,
                },
            )
            .await
            .map_err(|error| FlowFailure {
                stage: "udp_vmess_relay_chain",
                error,
                upstream: None,
            })?;
        dispatch.register_cached_protocol_flow_sender(Box::new(manager));

        Ok(FlowStartResult::Flow {
            outbound: Box::new(UdpFlowOutbound::Cached {
                tag: tag_owned,
                server: (*server).to_string(),
                port: *port,
            }),
            tx_bytes: 0,
        })
    }
}
