//! SOCKS5 outbound protocol implementation.
//!
//! TCP connect and relay-hop handshake stay here. SOCKS5 UDP association
//! runtime state lives in `crate::adapters::socks5::udp`.

use zero_core::Session;
use zero_engine::EngineError;
use zero_traits::TcpTunnelProtocol;

use crate::runtime::Proxy;
use crate::transport::{MeteredStream, TcpRelayStream};

/// Establish a SOCKS5 TCP upstream: dial the proxy server, run the SOCKS5
/// CONNECT tunnel handshake, return the relay stream.
///
/// Moved here from `runtime/upstream.rs` so the runtime dispatches via the
/// `ProtocolAdapter` trait instead of a per-protocol `connect_via_*` method.
pub(crate) async fn connect_tcp(
    proxy: &Proxy,
    session: &Session,
    server: &str,
    port: u16,
    auth: Option<(&str, &str)>,
) -> Result<TcpRelayStream, EngineError> {
    let upstream = proxy
        .protocols
        .direct_connector()
        .connect_host(server, port, proxy.resolver.as_ref())
        .await?;
    let mut upstream = MeteredStream::new(upstream);

    socks5::Socks5Outbound
        .establish_tcp_tunnel(
            &mut upstream,
            &socks5::Socks5TcpTunnelTarget {
                session,
                auth: auth
                    .map(|(username, password)| socks5::Socks5OutboundAuth { username, password }),
            },
        )
        .await?;
    proxy.record_session_outbound_traffic(session.id, upstream.drain_traffic());

    Ok(upstream.into_inner().into())
}

/// Apply a SOCKS5 tunnel handshake over an existing stream (relay hop).
///
/// Unlike [`connect_tcp`] this does not dial; the stream is already
/// connected to the SOCKS5 server through the preceding hop.
pub(crate) async fn apply_tcp_hop(
    _proxy: &Proxy,
    mut stream: TcpRelayStream,
    session: &Session,
    auth: Option<(&str, &str)>,
) -> Result<TcpRelayStream, EngineError> {
    socks5::Socks5Outbound
        .establish_tcp_tunnel(
            &mut stream,
            &socks5::Socks5TcpTunnelTarget {
                session,
                auth: auth
                    .map(|(username, password)| socks5::Socks5OutboundAuth { username, password }),
            },
        )
        .await
        .map_err(|e| EngineError::Io(std::io::Error::other(e)))?;
    Ok(stream)
}
