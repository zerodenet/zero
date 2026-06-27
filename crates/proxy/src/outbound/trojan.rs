//! Trojan outbound -?TCP connect.
//!
//! TCP outbound connect ([`connect_tcp`]) moved here from `runtime/upstream.rs`
//! so the runtime dispatches via the `ProtocolAdapter` trait. UDP stream-packet
//! management lives in the Trojan adapter UDP module.

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
pub(crate) async fn connect_tcp(
    request: TrojanTcpConnectRequest<'_>,
) -> Result<TcpRelayStream, EngineError> {
    let TrojanTcpConnectRequest {
        proxy,
        session,
        server,
        port,
        password,
        sni,
        insecure,
        client_fingerprint,
    } = request;

    let upstream = proxy
        .protocols
        .direct_connector()
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
    trojan::TrojanOutbound
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

pub(crate) struct TrojanTcpConnectRequest<'a> {
    pub proxy: &'a Proxy,
    pub session: &'a Session,
    pub server: &'a str,
    pub port: u16,
    pub password: &'a str,
    pub sni: Option<&'a str>,
    pub insecure: bool,
    pub client_fingerprint: Option<&'a str>,
}

/// Apply a Trojan tunnel handshake over an existing stream (relay hop).
/// Unlike [`connect_tcp`] this does not dial.
pub(crate) async fn apply_tcp_hop(
    _proxy: &Proxy,
    mut stream: TcpRelayStream,
    session: &Session,
    password: &str,
) -> Result<TcpRelayStream, EngineError> {
    trojan::TrojanOutbound
        .establish_tcp_tunnel(
            &mut stream,
            &trojan::TrojanTcpTunnelTarget { session, password },
        )
        .await
        .map_err(|e| EngineError::Io(std::io::Error::other(e)))?;
    Ok(stream)
}
