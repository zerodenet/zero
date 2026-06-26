//! Trojan TLS transport helpers.

use std::path::Path;

use zero_config::ClientTlsConfig;
use zero_engine::EngineError;
use zero_platform_tokio::{TcpRelayStream, TokioSocket};

pub struct TrojanUdpTlsOptions<'a> {
    pub tls_config: ClientTlsConfig,
    pub source_dir: Option<&'a Path>,
    pub server: &'a str,
}

pub async fn open_trojan_udp_tls_stream(
    socket: TokioSocket,
    options: TrojanUdpTlsOptions<'_>,
) -> Result<TcpRelayStream, EngineError> {
    crate::tls::connect_tls_upstream(
        socket,
        &options.tls_config,
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
        &options.tls_config,
        options.source_dir,
        options.server,
    )
    .await
}
