#![cfg(feature = "socks5")]

mod support;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::time::{timeout, Duration};
use zero_config::RuntimeConfig;
use zero_core::Address;
use zero_protocol_socks5::{build_udp_packet, parse_udp_packet};
use zero_proxy::Proxy as Engine;

use support::{free_port, free_udp_port, spawn_engine, wait_for_listener};
#[cfg(feature = "socks5")]
use support::{spawn_http_probe_server, wait_for, wait_for_group_selection};

#[cfg(feature = "socks5")]
#[path = "socks5_udp/relays_udp_through_fallback_group_when_primary_unreachable.rs"]
mod relays_udp_through_fallback_group_when_primary_unreachable;
#[cfg(feature = "socks5")]
#[path = "socks5_udp/relays_udp_through_nested_group_target.rs"]
mod relays_udp_through_nested_group_target;
#[path = "socks5_udp/relays_udp_through_socks5_udp_associate_direct_outbound.rs"]
mod relays_udp_through_socks5_udp_associate_direct_outbound;
#[cfg(feature = "socks5")]
#[path = "socks5_udp/relays_udp_through_upstream_socks5_udp_associate.rs"]
mod relays_udp_through_upstream_socks5_udp_associate;
#[cfg(feature = "socks5")]
#[path = "socks5_udp/relays_udp_through_urltest_group_after_probe_selects_direct.rs"]
mod relays_udp_through_urltest_group_after_probe_selects_direct;
#[cfg(feature = "socks5")]
#[path = "socks5_udp/relays_udp_through_urltest_group_with_nested_group_member.rs"]
mod relays_udp_through_urltest_group_with_nested_group_member;
