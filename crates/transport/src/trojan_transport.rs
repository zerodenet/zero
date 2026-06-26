//! Trojan transport helpers.

use std::path::Path;

use zero_config::ClientTlsConfig;
use zero_engine::EngineError;
use zero_platform_tokio::{TcpRelayStream, TokioSocket};

pub struct TrojanUdpTlsOptions<'a> {
    pub profile: trojan::TrojanUdpTlsProfile,
    pub source_dir: Option<&'a Path>,
    pub server: &'a str,
}

pub async fn open_trojan_udp_tls_stream(
    socket: TokioSocket,
    options: TrojanUdpTlsOptions<'_>,
) -> Result<TcpRelayStream, EngineError> {
    let tls_config = tls_config(options.profile);
    crate::tls::connect_tls_upstream(socket, &tls_config, options.source_dir, options.server).await
}

pub async fn open_trojan_udp_tls_relay_stream(
    stream: TcpRelayStream,
    options: TrojanUdpTlsOptions<'_>,
) -> Result<TcpRelayStream, EngineError> {
    let tls_config = tls_config(options.profile);
    crate::tls::connect_tls_stream(stream, &tls_config, options.source_dir, options.server).await
}

fn tls_config(tls_profile: trojan::TrojanUdpTlsProfile) -> ClientTlsConfig {
    ClientTlsConfig {
        server_name: tls_profile.server_name().map(|s| s.to_owned()),
        disable_sni: false,
        ca_cert_path: None,
        insecure: tls_profile.insecure(),
        alpn: Vec::new(),
        client_fingerprint: tls_profile.client_fingerprint().map(|s| s.to_owned()),
    }
}
