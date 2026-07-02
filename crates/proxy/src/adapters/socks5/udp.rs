use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::socks5::Socks5Adapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, ManagedRelayStart, UdpDispatch};
use crate::runtime::udp_flow::registered::UpstreamAssociationHandler;
use crate::runtime::Proxy;

mod active;
mod flow;
mod packet_path;
mod runtime;

pub(crate) fn upstream_association_handler() -> Box<dyn UpstreamAssociationHandler> {
    Box::<runtime::Socks5UdpRuntime>::default()
}

impl Socks5Adapter {
    pub(super) fn udp_packet_path_carrier_descriptor_impl(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::runtime::udp_flow::packet_path::PacketPathCarrierDescriptor> {
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
        let config =
            socks5::udp::Socks5UdpFlowConfig::new(tag, server, *port, *username, *password);
        Some(packet_path::carrier_descriptor(
            config.packet_path_spec().carrier_descriptor(),
        ))
    }

    pub(super) async fn build_udp_packet_path_impl(
        &self,
        proxy: &Proxy,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<
        std::sync::Arc<dyn crate::runtime::udp_flow::packet_path::PacketPathCarrier>,
        EngineError,
    > {
        let ResolvedLeafOutbound::Socks5 {
            tag,
            server,
            port,
            username,
            password,
        } = leaf
        else {
            return Err(EngineError::Io(std::io::Error::other(format!(
                "{} adapter received unsupported packet-path leaf: {leaf:?}",
                self.name()
            ))));
        };
        let config =
            socks5::udp::Socks5UdpFlowConfig::new(tag, server, *port, *username, *password);
        let carrier = config.packet_path_spec().carrier_build();
        packet_path::build(proxy, carrier).await
    }

    pub(super) async fn start_udp_flow_impl(
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
            return Err(FlowFailure {
                stage: "udp_unsupported_leaf",
                error: EngineError::Io(std::io::Error::other(format!(
                    "{} adapter received unsupported UDP leaf: {leaf:?}",
                    self.name()
                ))),
                upstream: None,
            });
        };
        let config =
            socks5::udp::Socks5UdpFlowConfig::new(tag, server, *port, *username, *password);
        flow::start(
            dispatch,
            ManagedRelayStart {
                proxy: Some(proxy),
                tag,
                session,
                carrier: None,
                tls_server_name: None,
                server,
                port: *port,
                resume: config.flow_resume(),
                payload,
            },
        )
        .await
    }
}
