//! UDP flow start: new outbound establishment.
//!
//! Contains [`UdpDispatch::start_flow`] (single-hop) and
//! [`UdpDispatch::start_relay_flow`] (multi-hop chain) for establishing new
//! UDP outbound connections, plus the chain resolution function
//! [`resolve_udp_packet_path_chain`].

use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

#[cfg(feature = "shadowsocks")]
use super::packet_path_chain::{PacketPathCarrierParams, PacketPathChainParams};
use super::{FlowFailure, FlowStartResult, UdpCandidate, UdpDispatch};
#[cfg(feature = "shadowsocks")]
use crate::runtime::udp_associate::sessions::UdpPacketPathCarrier;
#[cfg(feature = "vless")]
use crate::runtime::vless_udp::establish_vless_udp_upstream_over_stream;
use crate::runtime::Proxy;

// Re-exports consumed by `relay` submodule via `use super::*`.
#[allow(unused_imports)]
pub(super) use crate::runtime::udp_associate::sessions::UdpFlowOutbound;
#[allow(unused_imports)]
pub(super) use crate::runtime::udp_dispatch::{
    H2UdpPeer, MieruUdpPeer, SsUdpPeer, TrojanUdpPeer, UdpFlowContext, UdpPacketRef,
    UdpPeerEndpoint,
};
#[cfg(feature = "vmess")]
#[allow(unused_imports)]
pub(super) use crate::runtime::vmess_udp::{
    build_vmess_udp_transport_over_stream, establish_vmess_udp_upstream_over_stream,
    VmessUdpTransport,
};

// Chain resolution.

/// Resolve a relay chain into packet-path + datagram parameters.
///
/// Returns `Some` when the chain matches the "packet path carrier -> datagram
/// protocol" pattern. Currently recognises `[Shadowsocks, Shadowsocks]` and,
/// when the `socks5` feature is enabled, `[SOCKS5, Shadowsocks]`. Adding
/// new combinations only requires extending this function and implementing
/// [`UdpPacketPath`] + [`DatagramCodec`]: no new protocol-pair modules.
#[cfg(feature = "shadowsocks")]
fn resolve_udp_packet_path_chain<'a>(
    chain: &[ResolvedLeafOutbound<'a>],
) -> Option<PacketPathChainParams<'a>> {
    match chain {
        #[cfg(feature = "socks5")]
        [ResolvedLeafOutbound::Socks5 {
            tag: carrier_tag,
            server: carrier_server,
            port: carrier_port,
            username: carrier_username,
            password: carrier_password,
        }, ResolvedLeafOutbound::Shadowsocks {
            tag: datagram_tag,
            server: datagram_server,
            port: datagram_port,
            password: datagram_password,
            cipher: datagram_cipher,
        }] => Some(PacketPathChainParams {
            datagram_tag,
            carrier: PacketPathCarrierParams::Socks5 {
                tag: carrier_tag,
                server: carrier_server,
                port: *carrier_port,
                username: *carrier_username,
                password: *carrier_password,
            },
            datagram_server,
            datagram_port: *datagram_port,
            datagram_password,
            datagram_cipher,
        }),
        [ResolvedLeafOutbound::Shadowsocks {
            tag: carrier_tag,
            server: carrier_server,
            port: carrier_port,
            password: carrier_password,
            cipher: carrier_cipher,
        }, ResolvedLeafOutbound::Shadowsocks {
            tag: datagram_tag,
            server: datagram_server,
            port: datagram_port,
            password: datagram_password,
            cipher: datagram_cipher,
        }] => Some(PacketPathChainParams {
            datagram_tag,
            carrier: PacketPathCarrierParams::Shadowsocks {
                tag: carrier_tag,
                server: carrier_server,
                port: *carrier_port,
                password: carrier_password,
                cipher: carrier_cipher,
            },
            datagram_server,
            datagram_port: *datagram_port,
            datagram_password,
            datagram_cipher,
        }),
        #[cfg(feature = "hysteria2")]
        [ResolvedLeafOutbound::Hysteria2 {
            tag: carrier_tag,
            server: carrier_server,
            port: carrier_port,
            password: carrier_password,
            client_fingerprint: carrier_client_fingerprint,
            ..
        }, ResolvedLeafOutbound::Shadowsocks {
            tag: datagram_tag,
            server: datagram_server,
            port: datagram_port,
            password: datagram_password,
            cipher: datagram_cipher,
        }] => Some(PacketPathChainParams {
            datagram_tag,
            carrier: PacketPathCarrierParams::Hysteria2 {
                tag: carrier_tag,
                server: carrier_server,
                port: *carrier_port,
                password: carrier_password,
                client_fingerprint: *carrier_client_fingerprint,
            },
            datagram_server,
            datagram_port: *datagram_port,
            datagram_password,
            datagram_cipher,
        }),
        _ => None,
    }
}

#[cfg(feature = "shadowsocks")]
fn owned_packet_path_carrier(carrier: &PacketPathCarrierParams<'_>) -> UdpPacketPathCarrier {
    match carrier {
        #[cfg(feature = "socks5")]
        PacketPathCarrierParams::Socks5 {
            tag,
            server,
            port,
            username,
            password,
        } => UdpPacketPathCarrier::Socks5 {
            tag: (*tag).to_owned(),
            server: (*server).to_owned(),
            port: *port,
            username: username.map(ToOwned::to_owned),
            password: password.map(ToOwned::to_owned),
        },
        PacketPathCarrierParams::Shadowsocks {
            tag,
            server,
            port,
            password,
            cipher,
        } => UdpPacketPathCarrier::Shadowsocks {
            tag: (*tag).to_owned(),
            server: (*server).to_owned(),
            port: *port,
            password: (*password).to_owned(),
            cipher: (*cipher).to_owned(),
        },
        #[cfg(feature = "hysteria2")]
        PacketPathCarrierParams::Hysteria2 {
            tag,
            server,
            port,
            password,
            client_fingerprint,
        } => UdpPacketPathCarrier::Hysteria2 {
            tag: (*tag).to_owned(),
            server: (*server).to_owned(),
            port: *port,
            password: (*password).to_owned(),
            client_fingerprint: client_fingerprint.map(ToOwned::to_owned),
        },
    }
}

// impl UdpDispatch.

impl UdpDispatch {
    /// Start a new UDP flow by dispatching to the resolved outbound.
    pub(super) async fn start_flow(
        &mut self,
        proxy: &Proxy,
        candidate: UdpCandidate<'_>,
        session: &Session,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let candidate = match candidate {
            UdpCandidate::Leaf(candidate) => candidate,
            UdpCandidate::Relay(chain) => {
                return self.start_relay_flow(proxy, chain, session, payload).await;
            }
        };

        // Block is kernel-level (no adapter owns it): reject immediately.
        // Direct and every proxy protocol go through the adapter registry —
        // adding a protocol = register an adapter, zero changes here.
        if matches!(
            crate::runtime::orchestration::tcp_path_category(&candidate),
            crate::runtime::orchestration::TcpPathCategory::Block
        ) {
            return Ok(FlowStartResult::Blocked {
                tag: crate::runtime::orchestration::kernel_leaf_tag(&candidate)
                    .unwrap_or("block")
                    .to_string(),
            });
        }

        // Single dispatch: resolve the leaf to its adapter and start the flow.
        let adapter = proxy
            .protocols
            .find_outbound_leaf(&candidate)
            .map_err(|error| FlowFailure {
                stage: "find_outbound_leaf",
                error,
                upstream: None,
            })?;
        adapter
            .start_udp_flow(self, proxy, session, &candidate, payload)
            .await
    }
}

mod relay;
