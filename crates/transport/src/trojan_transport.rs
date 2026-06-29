//! Trojan TLS transport helpers.

use std::path::Path;

use zero_config::ClientTlsConfig;
use zero_engine::EngineError;
use zero_platform_tokio::{TcpRelayStream, TokioSocket};

#[derive(Debug, Clone)]
pub struct TrojanTlsProfile {
    server_name: Option<String>,
    insecure: bool,
    client_fingerprint: Option<String>,
}

impl TrojanTlsProfile {
    pub fn from_parts(
        server_name: Option<&str>,
        insecure: bool,
        client_fingerprint: Option<&str>,
    ) -> Self {
        Self {
            server_name: server_name.map(ToOwned::to_owned),
            insecure,
            client_fingerprint: client_fingerprint.map(ToOwned::to_owned),
        }
    }

    fn into_tls_config(self) -> ClientTlsConfig {
        ClientTlsConfig {
            server_name: self.server_name,
            disable_sni: false,
            ca_cert_path: None,
            insecure: self.insecure,
            alpn: Vec::new(),
            client_fingerprint: self.client_fingerprint,
        }
    }
}

pub struct TrojanUdpTlsOptions<'a> {
    pub tls_profile: TrojanTlsProfile,
    pub source_dir: Option<&'a Path>,
    pub server: &'a str,
}

pub async fn open_trojan_udp_tls_stream(
    socket: TokioSocket,
    options: TrojanUdpTlsOptions<'_>,
) -> Result<TcpRelayStream, EngineError> {
    crate::tls::connect_tls_upstream(
        socket,
        &options.tls_profile.into_tls_config(),
        options.source_dir,
        options.server,
    )
    .await
}

pub async fn open_trojan_udp_tls_relay_stream(
    stream: TcpRelayStream,
    options: TrojanUdpTlsOptions<'_>,
) -> Result<TcpRelayStream, EngineError> {
    crate::tls::connect_tls_stream(
        stream,
        &options.tls_profile.into_tls_config(),
        options.source_dir,
        options.server,
    )
    .await
}
