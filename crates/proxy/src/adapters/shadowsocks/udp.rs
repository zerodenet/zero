use std::sync::Arc;

use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::{unreachable_leaf, unreachable_udp_leaf};
use crate::adapters::shadowsocks::ShadowsocksAdapter;
use crate::protocol_adapter::ProtocolSupportCapability;
use crate::runtime::udp_dispatch::shadowsocks_flow::ShadowsocksDatagramSend;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
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
            crate::protocol_runtime::udp::packet_path_snapshot::packet_path_carrier_descriptor(
                shadowsocks::udp_cache_key(tag, server, *port, cipher, password),
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
        let cipher_kind = parse_shadowsocks_udp_cipher(
            cipher,
            "udp_shadowsocks_packet_path_carrier_cipher",
            Some((server, *port)),
        )
        .map_err(|failure| failure.error)?;
        let codec = Arc::new(shadowsocks::udp_flow_codec(
            cipher_kind,
            password.as_bytes(),
        ));
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
        let cipher_kind =
            parse_shadowsocks_udp_cipher(cipher, "udp_shadowsocks_datagram_source_cipher", None)
                .ok()?;
        let cache_key = shadowsocks::udp_cache_key(tag, server, *port, cipher, password);
        let codec = Arc::new(shadowsocks::udp_datagram_codec(
            cipher_kind,
            password.as_bytes(),
        ));
        Some(
            crate::protocol_runtime::udp::packet_path_snapshot::udp_datagram_source(
                tag, server, *port, cache_key, codec,
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
        let cipher_kind = parse_shadowsocks_udp_cipher(
            cipher,
            "udp_shadowsocks_parse_cipher",
            Some((server, *port)),
        )?;
        let cache_key = shadowsocks::udp_cache_key(tag, server, *port, cipher, password);
        dispatch
            .start_shadowsocks_datagram_flow(ShadowsocksDatagramSend {
                proxy,
                tag,
                session,
                server,
                port: *port,
                resume: shadowsocks::ShadowsocksUdpFlowResume::new(
                    cache_key,
                    cipher_kind,
                    password.as_bytes(),
                ),
                codec: Arc::new(shadowsocks::udp_flow_codec(
                    cipher_kind,
                    password.as_bytes(),
                )),
                payload,
            })
            .await
    }
}
