//! Shadowsocks outbound — TCP connect.
//!
//! TCP outbound connect ([`connect_tcp`]) moved here from `runtime/upstream.rs`
//! so the runtime dispatches via the `ProtocolAdapter` trait. UDP datagram
//! management lives in `crate::runtime::udp_dispatch::ss_manager`.

use zero_core::Session;
use zero_engine::EngineError;
use zero_traits::TcpSessionProtocol;

use crate::runtime::Proxy;
use crate::transport::{MeteredStream, TcpRelayStream};

/// Establish a Shadowsocks TCP upstream: dial the server, run the AEAD
/// session handshake, wrap the stream with the SS AEAD codec.
///
/// Moved from `runtime/upstream.rs`. The runtime dispatches via the adapter
/// trait instead of a per-protocol `connect_via_*` method.
pub(crate) async fn connect_tcp(
    request: ShadowsocksTcpConnectRequest<'_>,
) -> Result<TcpRelayStream, EngineError> {
    let ShadowsocksTcpConnectRequest {
        proxy,
        session,
        server,
        port,
        password,
        cipher,
    } = request;

    let upstream = proxy
        .protocols
        .direct_connector()
        .connect_host(server, port, proxy.resolver.as_ref())
        .await?;
    let mut metered = MeteredStream::new(upstream);
    let password_bytes = password.as_bytes().to_vec();
    let ss_session = <shadowsocks::ShadowsocksOutbound as TcpSessionProtocol<
        shadowsocks::ShadowsocksTcpTarget,
    >>::establish_tcp_session(
        &proxy.protocols.shadowsocks_outbound_protocol(),
        &mut metered,
        &shadowsocks::ShadowsocksTcpTarget {
            session,
            cipher,
            password: &password_bytes,
        },
    )
    .await?;
    proxy.record_session_outbound_traffic(session.id, metered.drain_traffic());
    Ok(wrap_outbound_stream(
        metered.into_inner().into(),
        ss_session,
        password_bytes,
    ))
}

pub(crate) struct ShadowsocksTcpConnectRequest<'a> {
    pub proxy: &'a Proxy,
    pub session: &'a Session,
    pub server: &'a str,
    pub port: u16,
    pub password: &'a str,
    pub cipher: shadowsocks::CipherKind,
}

/// Wrap a relay stream with the Shadowsocks AEAD outbound codec.
///
/// Used by both the direct TCP outbound ([`connect_tcp`]) and the relay-hop
/// applier ([`apply_tcp_hop`]) which re-establishes an SS session over an
/// existing stream.
pub(crate) fn wrap_outbound_stream(
    upstream: TcpRelayStream,
    ss_session: shadowsocks::ShadowsocksOutboundSession,
    password: Vec<u8>,
) -> TcpRelayStream {
    TcpRelayStream::new(shadowsocks::ShadowsocksAeadStream::outbound(
        upstream, ss_session, password,
    ))
}

/// Apply a Shadowsocks AEAD session handshake over an existing stream
/// (relay hop). Unlike [`connect_tcp`] this does not dial.
pub(crate) async fn apply_tcp_hop(
    mut stream: TcpRelayStream,
    session: &Session,
    password: &str,
    cipher: shadowsocks::CipherKind,
) -> Result<TcpRelayStream, EngineError> {
    let ss_session = <shadowsocks::ShadowsocksOutbound as TcpSessionProtocol<
        shadowsocks::ShadowsocksTcpTarget,
    >>::establish_tcp_session(
        &shadowsocks::ShadowsocksOutbound,
        &mut stream,
        &shadowsocks::ShadowsocksTcpTarget {
            session,
            cipher,
            password: password.as_bytes(),
        },
    )
    .await
    .map_err(|e| EngineError::Io(std::io::Error::other(e)))?;
    Ok(wrap_outbound_stream(
        stream,
        ss_session,
        password.as_bytes().to_vec(),
    ))
}
