mod support;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{sleep, timeout, Duration};
use zero_config::RuntimeConfig;
use zero_proxy::Proxy as Engine;

use support::{
    free_port, spawn_engine, spawn_http_probe_server, wait_for_group_selection, wait_for_listener,
};

#[path = "socks5/rejects_blocked_domain_via_route_rule.rs"]
mod rejects_blocked_domain_via_route_rule;
#[path = "socks5/relays_tcp_through_authenticated_socks5_upstream.rs"]
mod relays_tcp_through_authenticated_socks5_upstream;
#[path = "socks5/relays_tcp_through_fallback_group_when_primary_unreachable.rs"]
mod relays_tcp_through_fallback_group_when_primary_unreachable;
#[path = "socks5/relays_tcp_through_nested_group_target.rs"]
mod relays_tcp_through_nested_group_target;
#[path = "socks5/relays_tcp_through_selector_group_in_global_mode.rs"]
mod relays_tcp_through_selector_group_in_global_mode;
#[path = "socks5/relays_tcp_through_socks5_chained_outbound.rs"]
mod relays_tcp_through_socks5_chained_outbound;
#[path = "socks5/relays_tcp_through_socks5_direct_outbound.rs"]
mod relays_tcp_through_socks5_direct_outbound;
#[path = "socks5/relays_tcp_through_socks5_inbound_with_username_password_auth.rs"]
mod relays_tcp_through_socks5_inbound_with_username_password_auth;
#[path = "socks5/relays_tcp_through_urltest_group_after_probe_selects_direct.rs"]
mod relays_tcp_through_urltest_group_after_probe_selects_direct;
#[path = "socks5/relays_tcp_through_urltest_group_with_nested_group_member.rs"]
mod relays_tcp_through_urltest_group_with_nested_group_member;
