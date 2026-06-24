use std::sync::Arc;

use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::{unreachable_leaf, unreachable_udp_leaf};
use crate::adapters::socks5::Socks5Adapter;
use crate::protocol_adapter::ProtocolAdapter;
use crate::protocol_runtime::udp::ProtocolUdpFlowSnapshot;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, Socks5RelaySend, UdpDispatch};
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
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
        let auth = match (username, password) {
            (Some(user), Some(_)) => format!("|auth:{user}"),
            _ => String::new(),
        };
        Some(crate::protocol_runtime::udp::PacketPathCarrierDescriptor {
            cache_key: format!("socks5|{tag}|{server}:{port}{auth}"),
            server: (*server).to_string(),
            port: *port,
        })
    }

    #[cfg(feature = "shadowsocks")]
    pub(super) fn udp_packet_path_carrier_snapshot_impl(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::protocol_runtime::udp::UdpPacketPathCarrier> {
        use crate::protocol_runtime::udp::UdpPacketPathCarrier;

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
        let protocol = ProtocolUdpFlowSnapshot::Socks5 {
            username: (*username).map(|u| u.to_string()),
            password: (*password).map(|p| p.to_string()),
        };
        let sent = dispatch
            .send_socks5(Socks5RelaySend {
                proxy,
                tag,
                server,
                port: *port,
                protocol: &protocol,
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
            outbound: Box::new(UdpFlowOutbound::Relay {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                protocol,
            }),
            tx_bytes: sent as u64,
        })
    }
}
