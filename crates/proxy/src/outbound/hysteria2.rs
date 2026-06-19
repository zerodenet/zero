//! Hysteria2 outbound — TCP connect.
//!
//! TCP outbound connect ([`connect_tcp`]) moved here from `runtime/upstream.rs`
//! so the runtime dispatches via the `ProtocolAdapter` trait. UDP datagram
//! management lives in `crate::runtime::udp_dispatch::h2_manager`.

use zero_core::Session;
use zero_engine::EngineError;

use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

/// Establish a Hysteria2 TCP upstream via QUIC.
///
/// Moved from `runtime/upstream.rs`. The runtime dispatches via the adapter
/// trait instead of a per-protocol `connect_via_*` method.
pub(crate) async fn connect_tcp(
    _proxy: &Proxy,
    session: &Session,
    server: &str,
    port: u16,
    password: &str,
    client_fingerprint: Option<&str>,
) -> Result<TcpRelayStream, EngineError> {
    let connector = crate::transport::Hysteria2Connector::new(server, port, password)
        .with_fingerprint(client_fingerprint);
    let stream = connector.connect(session).await?;
    Ok(TcpRelayStream::new(stream))
}
