use std::io;

use zero_engine::{EngineError, RouteDecision};

use crate::transport::TcpRelayStream;

/// Unified result from the routing and outbound establishment pipeline.
pub(crate) struct TcpRouteResult {
    pub upstream: TcpRelayStream,
    pub outbound_tag: String,
    pub is_direct: bool,
    pub upstream_endpoint: Option<(String, u16)>,
    pub route_action: RouteDecision,
}

pub(crate) struct EstablishedTcpOutbound {
    pub(super) kind: EstablishedTcpOutboundKind,
}

pub(super) enum EstablishedTcpOutboundKind {
    Direct {
        tag: String,
        upstream: TcpRelayStream,
    },
    Block,
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
        Self {
            kind: EstablishedTcpOutboundKind::Direct {
                tag: tag.into(),
                upstream,
            },
        }
    }

    pub(crate) fn block(_tag: impl Into<String>) -> Self {
        Self {
            kind: EstablishedTcpOutboundKind::Block,
        }
    }

    pub(crate) fn proxied(
        tag: impl Into<String>,
        server: impl Into<String>,
        port: u16,
        upstream: TcpRelayStream,
    ) -> Self {
        Self {
            kind: EstablishedTcpOutboundKind::Proxied {
                tag: tag.into(),
                server: server.into(),
                port,
                upstream,
            },
        }
    }

    pub(crate) fn relay(upstream: TcpRelayStream) -> Self {
        Self {
            kind: EstablishedTcpOutboundKind::Relay { upstream },
        }
    }

    pub(crate) fn into_relay_stream(self) -> Result<TcpRelayStream, EngineError> {
        match self.kind {
            EstablishedTcpOutboundKind::Direct { upstream, .. }
            | EstablishedTcpOutboundKind::Proxied { upstream, .. }
            | EstablishedTcpOutboundKind::Relay { upstream } => Ok(upstream),
            EstablishedTcpOutboundKind::Block => Err(EngineError::Io(io::Error::new(
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
