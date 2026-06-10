#![cfg(feature = "socks5")]

mod support;

use socks5::{build_udp_packet, parse_udp_packet};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::time::{timeout, Duration};
use zero_config::RuntimeConfig;
use zero_core::Address;
use zero_proxy::Proxy as Engine;

use support::{free_port, free_udp_port, spawn_engine, wait_for_listener};
#[cfg(feature = "socks5")]
use support::{spawn_http_probe_server, wait_for, wait_for_group_selection};

#[cfg(feature = "socks5")]
#[path = "socks5_udp/relays_udp_through_fallback_group_when_primary_unreachable.rs"]
mod relays_udp_through_fallback_group_when_primary_unreachable;
#[cfg(all(feature = "socks5", feature = "hysteria2"))]
#[path = "socks5_udp/relays_udp_through_hysteria2_outbound.rs"]
mod relays_udp_through_hysteria2_outbound;
#[cfg(all(feature = "socks5", feature = "mieru"))]
#[path = "socks5_udp/relays_udp_through_mieru_outbound.rs"]
mod relays_udp_through_mieru_outbound;
#[cfg(feature = "mixed")]
#[path = "socks5_udp/relays_udp_through_mixed_udp_associate_direct_outbound.rs"]
mod relays_udp_through_mixed_udp_associate_direct_outbound;
#[cfg(feature = "socks5")]
#[path = "socks5_udp/relays_udp_through_nested_group_target.rs"]
mod relays_udp_through_nested_group_target;
#[cfg(all(feature = "socks5", feature = "shadowsocks"))]
#[path = "socks5_udp/relays_udp_through_shadowsocks_outbound.rs"]
mod relays_udp_through_shadowsocks_outbound;
#[cfg(all(feature = "socks5", feature = "shadowsocks"))]
#[path = "socks5_udp/relays_udp_through_shadowsocks_outbound_all_ciphers.rs"]
mod relays_udp_through_shadowsocks_outbound_all_ciphers;
#[cfg(all(feature = "socks5", feature = "shadowsocks"))]
#[path = "socks5_udp/relays_udp_through_shadowsocks_to_shadowsocks_relay_chain.rs"]
mod relays_udp_through_shadowsocks_to_shadowsocks_relay_chain;
#[cfg(all(feature = "socks5", feature = "mieru"))]
#[path = "socks5_udp/relays_udp_through_socks5_to_mieru_relay_chain.rs"]
mod relays_udp_through_socks5_to_mieru_relay_chain;
#[cfg(all(feature = "socks5", feature = "shadowsocks"))]
#[path = "socks5_udp/relays_udp_through_socks5_to_shadowsocks_relay_chain.rs"]
mod relays_udp_through_socks5_to_shadowsocks_relay_chain;
#[cfg(all(feature = "socks5", feature = "trojan"))]
#[path = "socks5_udp/relays_udp_through_socks5_to_trojan_relay_chain.rs"]
mod relays_udp_through_socks5_to_trojan_relay_chain;
#[cfg(all(feature = "socks5", feature = "vless"))]
#[path = "socks5_udp/relays_udp_through_socks5_to_vless_relay_chain.rs"]
mod relays_udp_through_socks5_to_vless_relay_chain;
#[cfg(all(feature = "socks5", feature = "vless"))]
#[path = "socks5_udp/relays_udp_through_socks5_to_vless_ws_relay_chain.rs"]
mod relays_udp_through_socks5_to_vless_ws_relay_chain;
#[path = "socks5_udp/relays_udp_through_socks5_udp_associate_direct_outbound.rs"]
mod relays_udp_through_socks5_udp_associate_direct_outbound;
#[cfg(all(feature = "socks5", feature = "trojan"))]
#[path = "socks5_udp/relays_udp_through_trojan_outbound.rs"]
mod relays_udp_through_trojan_outbound;
#[cfg(feature = "socks5")]
#[path = "socks5_udp/relays_udp_through_upstream_socks5_udp_associate.rs"]
mod relays_udp_through_upstream_socks5_udp_associate;
#[cfg(feature = "socks5")]
#[path = "socks5_udp/relays_udp_through_urltest_group_after_probe_selects_direct.rs"]
mod relays_udp_through_urltest_group_after_probe_selects_direct;
#[cfg(feature = "socks5")]
#[path = "socks5_udp/relays_udp_through_urltest_group_with_nested_group_member.rs"]
mod relays_udp_through_urltest_group_with_nested_group_member;
#[cfg(all(feature = "socks5", feature = "vless"))]
#[path = "socks5_udp/relays_udp_through_vless_outbound.rs"]
mod relays_udp_through_vless_outbound;
#[cfg(all(feature = "socks5", feature = "vmess"))]
#[path = "socks5_udp/relays_udp_through_vmess_to_vmess_relay_chain.rs"]
mod relays_udp_through_vmess_to_vmess_relay_chain;
