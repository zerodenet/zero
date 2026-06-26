use std::sync::Arc;

use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::{unreachable_leaf, unreachable_udp_leaf};
use crate::adapters::shadowsocks::ShadowsocksAdapter;
use crate::protocol_adapter::ProtocolSupportCapability;
use crate::protocol_runtime::udp::ProtocolUdpFlowResume;
use crate::runtime::udp_dispatch::shadowsocks_flow::ShadowsocksDatagramSend;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::Proxy;

impl ShadowsocksAdapter {
    pub(super) fn udp_packet_path_carrier_descriptor_impl(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::protocol_runtime::udp::PacketPathCarrierDescriptor> {
        let _ = self;
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
        let resume = shadowsocks::ShadowsocksUdpFlowResume::from_config(
            tag, server, *port, cipher, password,
        )
        .ok()?;
        Some(
            crate::protocol_runtime::udp::packet_path_snapshot::packet_path_carrier_descriptor(
                resume.packet_path_cache_key(),
                server,
                *port,
            ),
        )
    }

    pub(super) async fn build_udp_packet_path_impl(
        &self,
        proxy: &Proxy,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<Arc<dyn crate::protocol_runtime::udp::PacketPathCarrier>, EngineError> {
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
        let resume =
            shadowsocks::ShadowsocksUdpFlowResume::from_config("", server, *port, cipher, password)
                .map_err(|error| EngineError::Io(std::io::Error::other(error.to_string())))?;
        let codec = Arc::new(resume.packet_path_codec());
        crate::protocol_runtime::udp::packet_path_chain::carriers::udp_socket_carrier::build(
            proxy, server, *port, codec,
        )
        .await
    }

    pub(super) fn udp_datagram_source_impl<'a>(
        &self,
        leaf: &ResolvedLeafOutbound<'a>,
    ) -> Option<crate::protocol_runtime::udp::UdpDatagramSource<'a>> {
        let _ = self;
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
        let resume = shadowsocks::ShadowsocksUdpFlowResume::from_config(
            tag, server, *port, cipher, password,
        )
        .ok()?;
        let codec = Arc::new(resume.packet_path_codec());
        Some(
            crate::protocol_runtime::udp::packet_path_snapshot::udp_datagram_source(
                tag,
                server,
                *port,
                resume.packet_path_cache_key(),
                codec,
            ),
        )
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
        let resume = shadowsocks::ShadowsocksUdpFlowResume::from_config(
            tag, server, *port, cipher, password,
        )
        .map_err(|error| FlowFailure {
            stage: "udp_shadowsocks_resume",
            error: zero_engine::EngineError::Io(std::io::Error::other(error.to_string())),
            upstream: Some((server.to_string(), *port)),
        })?;
        dispatch
            .start_shadowsocks_datagram_flow(ShadowsocksDatagramSend {
                proxy,
                tag,
                session,
                server,
                port: *port,
                resume: ProtocolUdpFlowResume::Shadowsocks(resume),
                payload,
            })
            .await
    }
}
