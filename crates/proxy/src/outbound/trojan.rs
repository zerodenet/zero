//! Trojan outbound — TCP connect.
//!
//! TCP outbound connect ([`connect_tcp`]) moved here from `runtime/upstream.rs`
//! so the runtime dispatches via the `ProtocolAdapter` trait. UDP stream-packet
//! management lives in `crate::runtime::udp_dispatch::trojan_manager`.

use tokio::io::AsyncWriteExt;
use zero_config::ClientTlsConfig;
use zero_core::Session;
use zero_engine::EngineError;
use zero_traits::TcpTunnelProtocol;

use crate::runtime::Proxy;
use crate::transport::{MeteredStream, TcpRelayStream};

/// Establish a Trojan TCP upstream: dial the server, wrap in TLS, run the
/// Trojan tunnel handshake.
///
/// Moved from `runtime/upstream.rs`. The runtime dispatches via the adapter
/// trait instead of a per-protocol `connect_via_*` method.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn connect_tcp(
    proxy: &Proxy,
    session: &Session,
    server: &str,
    port: u16,
    password: &str,
    sni: Option<&str>,
    insecure: bool,
    client_fingerprint: Option<&str>,
) -> Result<TcpRelayStream, EngineError> {
    let upstream = proxy
        .protocols
        .direct_outbound
        .connect_host(server, port, proxy.resolver.as_ref())
        .await?;
    let tls_config = ClientTlsConfig {
        server_name: sni.map(|s| s.to_owned()),
        disable_sni: false,
        ca_cert_path: None,
        insecure,
        alpn: Vec::new(),
        client_fingerprint: client_fingerprint.map(|s| s.to_owned()),
    };
    let tls_stream = zero_transport::tls::connect_tls_upstream(
        upstream,
        &tls_config,
        proxy.config.source_dir(),
        server,
    )
    .await?;
    let mut metered = MeteredStream::new(tls_stream);
    proxy
        .protocols
        .trojan_outbound
        .establish_tcp_tunnel(
            &mut metered,
            &trojan::TrojanTcpTunnelTarget { session, password },
        )
        .await?;
    metered.flush().await?;
    let traffic = metered.drain_traffic();
    tracing::debug!(
        session_id = session.id,
        trojan_handshake_tx = traffic.written_bytes,
        target = ?session.target,
        target_port = session.port,
        "trojan upstream connected"
    );
    proxy.record_session_outbound_traffic(session.id, traffic);
    Ok(metered.into_inner())
}

/// Apply a Trojan tunnel handshake over an existing stream (relay hop).
/// Unlike [`connect_tcp`] this does not dial.
pub(crate) async fn apply_tcp_hop(
    proxy: &Proxy,
    mut stream: TcpRelayStream,
    session: &Session,
    password: &str,
) -> Result<TcpRelayStream, EngineError> {
    proxy
        .protocols
        .trojan_outbound
        .establish_tcp_tunnel(
            &mut stream,
            &trojan::TrojanTcpTunnelTarget { session, password },
        )
        .await
        .map_err(|e| EngineError::Io(std::io::Error::other(e)))?;
    Ok(stream)
}
