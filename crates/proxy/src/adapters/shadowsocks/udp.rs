use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::{unreachable_leaf, unreachable_udp_leaf};
use crate::adapters::shadowsocks::ShadowsocksAdapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::managed::ManagedDatagramFlowHandler;
use crate::runtime::Proxy;

mod flow;
mod managed;
mod packet_path;

pub(crate) struct ShadowsocksUdpFlowStart<'a> {
    pub(crate) tag: &'a str,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: shadowsocks::udp::ShadowsocksUdpFlowResume,
}

pub(crate) fn managed_datagram_handler() -> Box<dyn ManagedDatagramFlowHandler> {
    managed::handler()
}

impl ShadowsocksAdapter {
    pub(super) fn udp_packet_path_carrier_descriptor_impl(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::runtime::udp_flow::packet_path::PacketPathCarrierDescriptor> {
        let ResolvedLeafOutbound::Shadowsocks {
            tag,
            server,
            port,
            password,
            cipher,
        } = leaf
        else {
            return None;
        };
        let descriptor = shadowsocks::udp::udp_packet_path_carrier_descriptor_from_config(
            tag, server, *port, cipher, password,
        )
        .ok()?;
        Some(packet_path::carrier_descriptor(descriptor))
    }

    pub(super) async fn build_udp_packet_path_impl(
        &self,
        proxy: &Proxy,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<
        std::sync::Arc<dyn crate::runtime::udp_flow::packet_path::PacketPathCarrier>,
        EngineError,
    > {
        let ResolvedLeafOutbound::Shadowsocks {
            server,
            port,
            password,
            cipher,
            ..
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf).error);
        };
        let codec = shadowsocks::udp::udp_packet_path_carrier_codec_from_config(
            "", server, *port, cipher, password,
        )
        .map_err(|error| EngineError::Io(std::io::Error::other(error.to_string())))?;
        packet_path::build(proxy, server, *port, codec).await
    }

    pub(super) fn udp_datagram_source_impl(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::runtime::udp_flow::packet_path::UdpDatagramSource> {
        let ResolvedLeafOutbound::Shadowsocks {
            tag,
            server,
            port,
            password,
            cipher,
        } = leaf
        else {
            return None;
        };
        let datagram = shadowsocks::udp::udp_packet_path_datagram_source_build_from_config(
            tag, server, *port, cipher, password,
        )
        .ok()?;
        Some(packet_path::datagram_source(datagram))
    }

    pub(super) async fn start_udp_flow_impl(
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
        let resume: shadowsocks::udp::ShadowsocksUdpFlowResume =
            shadowsocks::udp::udp_flow_resume_from_config(tag, server, *port, cipher, password)
                .map_err(|error| FlowFailure {
                    stage: "udp_shadowsocks_resume",
                    error: EngineError::Io(std::io::Error::other(error.to_string())),
                    upstream: Some((server.to_string(), *port)),
                })?;
        let request = ShadowsocksUdpFlowStart {
            tag,
            server,
            port: *port,
            resume,
        };
        flow::start(dispatch, proxy, session, request, payload).await
    }
}
