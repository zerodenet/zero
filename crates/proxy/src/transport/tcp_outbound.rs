//! TCP outbound data types used by both transport and runtime layers.
//!
//! The TCP pipe orchestration lives in `crate::runtime::tcp_dispatch`.
//! This module only defines the data types exchanged between runtime
//! orchestration and transport relay code.

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
    Proxied {
        tag: String,
        server: String,
        port: u16,
        upstream: TcpRelayStream,
    },
    Relay {
        upstream: TcpRelayStream,
    },
}

impl EstablishedTcpOutbound {
    pub(crate) fn direct(tag: impl Into<String>, upstream: TcpRelayStream) -> Self {
        Self::Direct {
            tag: tag.into(),
            upstream,
        }
    }

    pub(crate) fn block(tag: impl Into<String>) -> Self {
        Self::Block { tag: tag.into() }
    }

    pub(crate) fn proxied(
        tag: impl Into<String>,
        server: impl Into<String>,
        port: u16,
        upstream: TcpRelayStream,
    ) -> Self {
        Self::Proxied {
            tag: tag.into(),
            server: server.into(),
            port,
            upstream,
        }
    }

    pub(crate) fn relay(upstream: TcpRelayStream) -> Self {
        Self::Relay { upstream }
    }

    pub(crate) fn into_relay_stream(self) -> Result<TcpRelayStream, EngineError> {
        match self {
            Self::Direct { upstream, .. }
            | Self::Proxied { upstream, .. }
            | Self::Relay { upstream } => Ok(upstream),
            Self::Block { .. } => Err(EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "first relay hop resolved to block",
            ))),
        }
    }
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
        EstablishedTcpOutbound::Proxied {
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
