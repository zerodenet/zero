//! Trojan outbound -?TCP connect.
//!
//! TCP outbound connect ([`connect_tcp`]) moved here from `runtime/upstream.rs`
//! so the runtime dispatches via registered TCP outbound capabilities. UDP stream-packet
//! management lives in the Trojan adapter UDP module.

use tokio::io::AsyncWriteExt;
use zero_config::ClientTlsConfig;
use zero_core::Session;
use zero_engine::EngineError;
use zero_traits::TcpTunnelProtocol;

use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::Proxy;
use crate::transport::{
    open_trojan_udp_tls_relay_stream, open_trojan_udp_tls_stream, MeteredStream, TcpRelayStream,
    TrojanUdpTlsOptions,
};

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
    let tls_stream = open_trojan_udp_tls_stream(
        upstream,
        trojan_tls_options(
            proxy,
            server,
            trojan_tcp_tls_config(sni, insecure, client_fingerprint),
        ),
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

pub(crate) async fn open_udp_tls_stream(
    proxy: &Proxy,
    endpoint: OutboundEndpoint<'_>,
    resume: &trojan::TrojanUdpFlowResume,
) -> Result<TcpRelayStream, EngineError> {
    let upstream = proxy
        .protocols
        .direct_connector()
        .connect_host(endpoint.server, endpoint.port, proxy.resolver.as_ref())
        .await?;

    open_trojan_udp_tls_stream(
        upstream,
        udp_tls_options(proxy, endpoint, resume.tls_profile(None)),
    )
    .await
}

pub(crate) async fn open_udp_tls_relay_stream(
    stream: TcpRelayStream,
    tls_server_name: Option<&str>,
    proxy: &Proxy,
    endpoint: OutboundEndpoint<'_>,
    resume: &trojan::TrojanUdpFlowResume,
) -> Result<TcpRelayStream, EngineError> {
    open_trojan_udp_tls_relay_stream(
        stream,
        udp_tls_options(proxy, endpoint, resume.tls_profile(tls_server_name)),
    )
    .await
}

fn udp_tls_options<'a>(
    proxy: &'a Proxy,
    endpoint: OutboundEndpoint<'a>,
    tls_profile: trojan::TrojanUdpTlsProfile,
) -> TrojanUdpTlsOptions<'a> {
    trojan_tls_options(proxy, endpoint.server, udp_tls_config(tls_profile))
}

fn trojan_tls_options<'a>(
    proxy: &'a Proxy,
    server: &'a str,
    tls_config: ClientTlsConfig,
) -> TrojanUdpTlsOptions<'a> {
    TrojanUdpTlsOptions {
        tls_config,
        source_dir: proxy.config.source_dir(),
        server,
    }
}

fn trojan_tcp_tls_config(
    sni: Option<&str>,
    insecure: bool,
    client_fingerprint: Option<&str>,
) -> ClientTlsConfig {
    ClientTlsConfig {
        server_name: sni.map(ToOwned::to_owned),
        disable_sni: false,
        ca_cert_path: None,
        insecure,
        alpn: Vec::new(),
        client_fingerprint: client_fingerprint.map(ToOwned::to_owned),
    }
}

fn udp_tls_config(tls_profile: trojan::TrojanUdpTlsProfile) -> ClientTlsConfig {
    ClientTlsConfig {
        server_name: tls_profile.server_name().map(ToOwned::to_owned),
        disable_sni: false,
        ca_cert_path: None,
        insecure: tls_profile.insecure(),
        alpn: Vec::new(),
        client_fingerprint: tls_profile.client_fingerprint().map(ToOwned::to_owned),
    }
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
