use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::Proxy;
use crate::transport::{
    open_trojan_udp_tls_relay_stream, open_trojan_udp_tls_stream, TcpRelayStream,
    TrojanUdpTlsOptions,
};
use zero_config::ClientTlsConfig;
use zero_engine::EngineError;

pub(super) async fn direct_tls_stream(
    proxy: &Proxy,
    endpoint: OutboundEndpoint<'_>,
    resume: &trojan::TrojanUdpFlowResume,
) -> Result<TcpRelayStream, EngineError> {
    let tls_profile = resume.tls_profile(None);
    let upstream = proxy
        .protocols
        .direct_connector()
        .connect_host(endpoint.server, endpoint.port, proxy.resolver.as_ref())
        .await?;

    let tls_stream = open_trojan_udp_tls_stream(
        upstream,
        TrojanUdpTlsOptions {
            tls_config: tls_config(tls_profile),
            source_dir: proxy.config.source_dir(),
            server: endpoint.server,
        },
    )
    .await?;

    Ok(tls_stream)
}

pub(super) async fn relay_tls_stream(
    stream: TcpRelayStream,
    tls_server_name: Option<&str>,
    proxy: &Proxy,
    endpoint: OutboundEndpoint<'_>,
    resume: &trojan::TrojanUdpFlowResume,
) -> Result<TcpRelayStream, EngineError> {
    let tls_profile = resume.tls_profile(tls_server_name);
    open_trojan_udp_tls_relay_stream(
        stream,
        TrojanUdpTlsOptions {
            tls_config: tls_config(tls_profile),
            source_dir: proxy.config.source_dir(),
            server: endpoint.server,
        },
    )
    .await
}

fn tls_config(tls_profile: trojan::TrojanUdpTlsProfile) -> ClientTlsConfig {
    ClientTlsConfig {
        server_name: tls_profile.server_name().map(ToOwned::to_owned),
        disable_sni: false,
        ca_cert_path: None,
        insecure: tls_profile.insecure(),
        alpn: Vec::new(),
        client_fingerprint: tls_profile.client_fingerprint().map(ToOwned::to_owned),
    }
}
