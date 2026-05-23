//! TCP outbound data types used by both transport and runtime layers.
//!
//! The runtime orchestration (route_and_establish_tcp, establish_tcp_outbound,
//! establish_tcp_candidate, establish_relay_chain) lives in `crate::runtime::tcp_outbound`.

use std::io;

use crate::transport::stream::TcpRelayStream;
use zero_engine::{EngineError, RouteDecision};

/// Unified result from the routing and outbound establishment pipeline.
pub(crate) struct TcpRouteResult {
    pub upstream: TcpRelayStream,
    pub outbound_tag: String,
    pub is_direct: bool,
    pub upstream_endpoint: Option<(String, u16)>,
    pub route_action: RouteDecision,
}

#[allow(dead_code)]
pub(crate) enum EstablishedTcpOutbound {
    Direct {
        tag: String,
        upstream: TcpRelayStream,
    },
    Block {
        tag: String,
    },
    Socks5 {
        tag: String,
        server: String,
        port: u16,
        upstream: TcpRelayStream,
    },
    Vless {
        tag: String,
        server: String,
        port: u16,
        upstream: TcpRelayStream,
    },
    Hysteria2 {
        tag: String,
        server: String,
        port: u16,
        upstream: TcpRelayStream,
    },
    Shadowsocks {
        tag: String,
        server: String,
        port: u16,
        upstream: TcpRelayStream,
    },
    Trojan {
        tag: String,
        server: String,
        port: u16,
        upstream: TcpRelayStream,
    },
    Vmess {
        tag: String,
        server: String,
        port: u16,
        upstream: TcpRelayStream,
    },
    Relay {
        upstream: TcpRelayStream,
    },
}

pub(crate) struct TcpOutboundFailure {
    #[allow(dead_code)]
    pub stage: &'static str,
    pub error: EngineError,
    #[allow(dead_code)]
    pub upstream_endpoint: Option<(String, u16)>,
}

/// Extract the upstream stream and metadata from an EstablishedTcpOutbound.
/// Maps Block to a "connection refused" error.
pub(crate) fn extract_tcp_stream(
    outbound: EstablishedTcpOutbound,
) -> Result<TcpRouteResult, EngineError> {
    match outbound {
        EstablishedTcpOutbound::Direct { tag, upstream } => Ok(TcpRouteResult {
            upstream,
            outbound_tag: tag,
            is_direct: true,
            upstream_endpoint: None,
            route_action: RouteDecision::Direct,
        }),
        EstablishedTcpOutbound::Block { .. } => Err(EngineError::Io(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            "blocked",
        ))),
        EstablishedTcpOutbound::Socks5 {
            tag,
            server,
            port,
            upstream,
        }
        | EstablishedTcpOutbound::Vless {
            tag,
            server,
            port,
            upstream,
        }
        | EstablishedTcpOutbound::Hysteria2 {
            tag,
            server,
            port,
            upstream,
        }
        | EstablishedTcpOutbound::Shadowsocks {
            tag,
            server,
            port,
            upstream,
        }
        | EstablishedTcpOutbound::Trojan {
            tag,
            server,
            port,
            upstream,
        }
        | EstablishedTcpOutbound::Vmess {
            tag,
            server,
            port,
            upstream,
        } => Ok(TcpRouteResult {
            upstream,
            outbound_tag: tag,
            is_direct: false,
            upstream_endpoint: Some((server, port)),
            route_action: RouteDecision::Direct,
        }),
        EstablishedTcpOutbound::Relay { upstream } => Ok(TcpRouteResult {
            upstream,
            outbound_tag: "relay".to_owned(),
            is_direct: false,
            upstream_endpoint: None,
            route_action: RouteDecision::Direct,
        }),
    }
}
