use std::net::{TcpStream, ToSocketAddrs};

use zero_core::Address;
use zero_router::{RouteAction, RouteContext};

use super::Engine;
use crate::{EngineError, ResolvedLeafOutbound, ResolvedOutbound};

impl Engine {
    pub fn dns_lookup(&self, hostname: &str) -> Result<serde_json::Value, EngineError> {
        let addrs: Vec<String> = format!("{hostname}:0")
            .to_socket_addrs()
            .map_err(|error| EngineError::Io(std::io::Error::other(error)))?
            .map(|address| address.ip().to_string())
            .collect();

        Ok(serde_json::json!({
            "hostname": hostname,
            "resolved_addresses": addrs,
            "count": addrs.len(),
        }))
    }

    pub fn trace_route(
        &self,
        target: &str,
        port: u16,
        protocol: &str,
        inbound_tag: Option<&str>,
    ) -> Result<serde_json::Value, EngineError> {
        let address = match target.parse::<std::net::IpAddr>() {
            Ok(std::net::IpAddr::V4(value)) => Address::Ipv4(value.octets()),
            Ok(std::net::IpAddr::V6(value)) => Address::Ipv6(value.octets()),
            Err(_) => Address::Domain(target.to_owned()),
        };
        let decision = self
            .router
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .decide_trace_with_context(RouteContext {
                address: &address,
                sni: None,
                inbound_tag,
            });
        let matched_rule = decision.matched_rule.map(
            |matched| serde_json::json!({"index": matched.index, "condition": matched.condition}),
        );

        Ok(serde_json::json!({
            "target": target,
            "port": port,
            "protocol": protocol,
            "inbound_tag": inbound_tag,
            "effective_mode": self.mode_kind(),
            "route_action": match &decision.action {
                RouteAction::Route(tag) => serde_json::json!({"route": tag}),
                RouteAction::Direct => serde_json::json!("direct"),
                RouteAction::Reject => serde_json::json!("reject"),
            },
            "matched_rule": matched_rule,
        }))
    }

    pub fn probe_target(&self, target_tag: &str) -> Result<serde_json::Value, EngineError> {
        let plan = self.plan();
        let target_id =
            plan.target_id(target_tag)
                .ok_or_else(|| EngineError::SelectorGroupNotFound {
                    tag: target_tag.to_owned(),
                })?;
        let (resolved, _plan) = self.resolve_target_id(target_id).ok_or_else(|| {
            EngineError::SelectorGroupNotFound {
                tag: target_tag.to_owned(),
            }
        })?;
        let (host, port) = match &resolved {
            ResolvedOutbound::Single(leaf) => extract_target_addr(leaf),
            ResolvedOutbound::Fallback { candidates } => candidates
                .first()
                .map(extract_target_addr)
                .unwrap_or((None, None)),
            ResolvedOutbound::Relay { .. } => (None, None),
        };
        let (Some(host), Some(port)) = (host, port) else {
            return Ok(serde_json::json!({
                "target_tag": target_tag,
                "reachable": false,
                "error": "outbound has no probeable fixed server",
            }));
        };

        let started = std::time::Instant::now();
        let reachable = format!("{host}:{port}")
            .to_socket_addrs()
            .ok()
            .and_then(|mut addresses| addresses.next())
            .is_some_and(|address| {
                TcpStream::connect_timeout(&address, std::time::Duration::from_secs(2)).is_ok()
            });
        Ok(serde_json::json!({
            "target_tag": target_tag,
            "server": host,
            "port": port,
            "reachable": reachable,
            "latency_ms": reachable.then(|| started.elapsed().as_millis() as u64),
        }))
    }
}

fn extract_target_addr(leaf: &ResolvedLeafOutbound<'_>) -> (Option<String>, Option<u16>) {
    match leaf {
        ResolvedLeafOutbound::Direct { .. } | ResolvedLeafOutbound::Block { .. } => (None, None),
        ResolvedLeafOutbound::Socks5 { server, port, .. }
        | ResolvedLeafOutbound::Vless { server, port, .. }
        | ResolvedLeafOutbound::Hysteria2 { server, port, .. }
        | ResolvedLeafOutbound::Shadowsocks { server, port, .. }
        | ResolvedLeafOutbound::Trojan { server, port, .. }
        | ResolvedLeafOutbound::Vmess { server, port, .. }
        | ResolvedLeafOutbound::Mieru { server, port, .. } => {
            (Some(server.to_string()), Some(*port))
        }
    }
}
