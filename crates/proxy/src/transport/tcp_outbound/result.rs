use std::io;

use zero_engine::{EngineError, RouteDecision};

use super::model::{EstablishedTcpOutbound, EstablishedTcpOutboundKind, TcpRouteResult};

/// Extract the upstream stream and metadata from an EstablishedTcpOutbound.
/// Maps Block to a "connection refused" error.
pub(crate) fn extract_tcp_stream(
    outbound: EstablishedTcpOutbound,
) -> Result<TcpRouteResult, EngineError> {
    match outbound.kind {
        EstablishedTcpOutboundKind::Direct { tag, upstream } => Ok(TcpRouteResult {
            upstream,
            outbound_tag: tag,
            is_direct: true,
            upstream_endpoint: None,
            route_action: RouteDecision::Direct,
        }),
        EstablishedTcpOutboundKind::Block => Err(EngineError::Io(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            "blocked",
        ))),
        EstablishedTcpOutboundKind::Proxied {
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
        EstablishedTcpOutboundKind::Relay { upstream } => Ok(TcpRouteResult {
            upstream,
            outbound_tag: "relay".to_owned(),
            is_direct: false,
            upstream_endpoint: None,
            route_action: RouteDecision::Direct,
        }),
    }
}
