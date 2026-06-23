use std::sync::Arc;

use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::{unreachable_leaf, unreachable_udp_leaf};
use crate::adapters::shadowsocks::ShadowsocksAdapter;
use crate::protocol_adapter::ProtocolAdapter;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
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
        Some(crate::protocol_runtime::udp::PacketPathCarrierDescriptor {
            cache_key: format!("shadowsocks|{tag}|{server}:{port}|{cipher}|{password}"),
            server: (*server).to_string(),
            port: *port,
        })
    }

    pub(super) fn udp_packet_path_carrier_snapshot_impl(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::protocol_runtime::udp::UdpPacketPathCarrier> {
        use crate::protocol_runtime::udp::UdpPacketPathCarrier;

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
        Some(UdpPacketPathCarrier::Shadowsocks {
            cache_key: format!("shadowsocks|{tag}|{server}:{port}|{cipher}|{password}"),
            tag: (*tag).to_string(),
            server: (*server).to_string(),
            port: *port,
            password: (*password).to_string(),
            cipher: (*cipher).to_string(),
        })
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
        crate::protocol_runtime::udp::build_shadowsocks_packet_path(
            proxy, server, *port, password, cipher,
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
        Some(crate::protocol_runtime::udp::UdpDatagramSource {
            tag,
            server,
            port: *port,
            password,
            cipher,
        })
    }

    pub(super) async fn start_udp_flow_impl(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        use crate::protocol_runtime::udp::ShadowsocksUdpFlow;

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
        let (protocol_state, chain_tasks) = dispatch.protocol_parts();
        let sent = protocol_state
            .start_shadowsocks_udp_flow(
                chain_tasks,
                ShadowsocksUdpFlow {
                    proxy,
                    session,
                    server,
                    port: *port,
                    password,
                    cipher,
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
            outbound: Box::new(UdpFlowOutbound::Shadowsocks {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                password: (*password).to_string(),
                cipher: (*cipher).to_string(),
                packet_path_carrier: None,
            }),
            tx_bytes: sent as u64,
        })
    }
}
