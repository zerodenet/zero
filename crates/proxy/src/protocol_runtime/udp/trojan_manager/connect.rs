use super::super::TrojanUdpPeer;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use zero_config::ClientTlsConfig;
use zero_engine::EngineError;

pub(super) async fn direct_tls_stream(
    proxy: &Proxy,
    peer: &TrojanUdpPeer<'_>,
) -> Result<TcpRelayStream, EngineError> {
    let tls_profile = peer.resume.tls_profile(None);
    let upstream = proxy
        .protocols
        .direct_connector()
        .connect_host(
            peer.endpoint.server,
            peer.endpoint.port,
            proxy.resolver.as_ref(),
        )
        .await?;

    let tls_stream = zero_transport::tls::connect_tls_upstream(
        upstream,
        &tls_config(tls_profile),
        proxy.config.source_dir(),
        peer.endpoint.server,
    )
    .await?;

    Ok(TcpRelayStream::new(tls_stream))
}

pub(super) async fn relay_tls_stream(
    stream: TcpRelayStream,
    tls_server_name: Option<&str>,
    proxy: &Proxy,
    peer: &TrojanUdpPeer<'_>,
) -> Result<TcpRelayStream, EngineError> {
    let tls_profile = peer.resume.tls_profile(tls_server_name);
    zero_transport::tls::connect_tls_stream(
        stream,
        &tls_config(tls_profile),
        proxy.config.source_dir(),
        peer.endpoint.server,
    )
    .await
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
