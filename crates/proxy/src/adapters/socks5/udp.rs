use std::sync::Arc;

use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::{unreachable_leaf, unreachable_udp_leaf};
use crate::adapters::socks5::Socks5Adapter;
use crate::protocol_adapter::ProtocolSupportCapability;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, Socks5RelaySend, UdpDispatch};
use crate::runtime::Proxy;

impl Socks5Adapter {
    #[cfg(feature = "shadowsocks")]
    pub(super) fn udp_packet_path_carrier_descriptor_impl(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::protocol_runtime::udp::PacketPathCarrierDescriptor> {
        let _ = self;
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
        Some(
            crate::protocol_runtime::udp::socks5_packet_path_carrier_descriptor(
                tag,
                server,
                *port,
                username.zip(*password).map(|(user, _)| user),
            ),
        )
    }

    #[cfg(feature = "shadowsocks")]
    pub(super) fn udp_packet_path_carrier_snapshot_impl(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::protocol_runtime::udp::UdpPacketPathCarrier> {
        let _ = self;
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
        Some(
            crate::protocol_runtime::udp::socks5_packet_path_carrier_snapshot(
                tag,
                server,
                *port,
                username.zip(*password).map(|(user, _)| user),
                *password,
            ),
        )
    }

    #[cfg(feature = "shadowsocks")]
    pub(super) async fn build_udp_packet_path_impl(
        &self,
        proxy: &Proxy,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<Arc<dyn crate::protocol_runtime::udp::PacketPathCarrier>, EngineError> {
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
        crate::protocol_runtime::socks5_udp::build_socks5_packet_path(
            proxy,
            tag,
            server,
            *port,
            username.zip(*password),
        )
        .await
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
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        dispatch
            .start_socks5_relay_flow(Socks5RelaySend {
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
    }
}
