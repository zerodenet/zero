use std::sync::Arc;

use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::{unreachable_leaf, unreachable_udp_leaf};
use crate::adapters::shadowsocks::ShadowsocksAdapter;
use crate::protocol_adapter::ProtocolAdapter;
use crate::protocol_runtime::udp::ProtocolUdpFlowSnapshot;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
use crate::runtime::Proxy;

fn parse_shadowsocks_udp_cipher(
    cipher: &str,
    stage: &'static str,
    upstream: Option<(&str, u16)>,
) -> Result<shadowsocks::CipherKind, FlowFailure> {
    shadowsocks::CipherKind::from_str(cipher).ok_or_else(|| FlowFailure {
        stage,
        error: EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("unknown shadowsocks cipher: {cipher}"),
        )),
        upstream: upstream.map(|(server, port)| (server.to_string(), port)),
    })
}

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
        Some(
            crate::protocol_runtime::udp::shadowsocks_packet_path_carrier_descriptor(
                tag, server, *port, cipher, password,
            ),
        )
    }

    pub(super) fn udp_packet_path_carrier_snapshot_impl(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::protocol_runtime::udp::UdpPacketPathCarrier> {
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
        Some(
            crate::protocol_runtime::udp::shadowsocks_packet_path_carrier_snapshot(
                tag, server, *port, cipher, password,
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
        let cipher_kind = parse_shadowsocks_udp_cipher(
            cipher,
            "udp_shadowsocks_packet_path_carrier_cipher",
            Some((server, *port)),
        )
        .map_err(|failure| failure.error)?;
        crate::protocol_runtime::udp::build_shadowsocks_packet_path(
            proxy,
            server,
            *port,
            password,
            cipher_kind,
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
        let cipher_kind =
            parse_shadowsocks_udp_cipher(cipher, "udp_shadowsocks_datagram_source_cipher", None)
                .ok()?;
        Some(crate::protocol_runtime::udp::UdpDatagramSource {
            tag,
            server,
            port: *port,
            password,
            datagram_cache_key: crate::protocol_runtime::udp::shadowsocks_udp_cache_key(
                tag, server, *port, cipher, password,
            ),
            cipher_kind,
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
        let cipher_kind = parse_shadowsocks_udp_cipher(
            cipher,
            "udp_shadowsocks_parse_cipher",
            Some((server, *port)),
        )?;
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
                    cipher: cipher_kind,
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
            outbound: Box::new(UdpFlowOutbound::Datagram {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                protocol: ProtocolUdpFlowSnapshot::Shadowsocks {
                    password: (*password).to_string(),
                    datagram_cache_key: crate::protocol_runtime::udp::shadowsocks_udp_cache_key(
                        tag, server, *port, cipher, password,
                    ),
                    cipher_kind,
                    packet_path_carrier: None,
                },
            }),
            tx_bytes: sent as u64,
        })
    }
}
