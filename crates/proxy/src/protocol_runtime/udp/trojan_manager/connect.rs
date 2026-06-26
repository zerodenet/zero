use super::super::TrojanUdpPeer;
use crate::runtime::Proxy;
use crate::transport::{
    open_trojan_udp_tls_relay_stream, open_trojan_udp_tls_stream, TcpRelayStream,
    TrojanUdpTlsOptions,
};
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

    let tls_stream = open_trojan_udp_tls_stream(
        upstream,
        TrojanUdpTlsOptions {
            profile: tls_profile,
            source_dir: proxy.config.source_dir(),
            server: peer.endpoint.server,
        },
    )
    .await?;

    Ok(tls_stream)
}

pub(super) async fn relay_tls_stream(
    stream: TcpRelayStream,
    tls_server_name: Option<&str>,
    proxy: &Proxy,
    peer: &TrojanUdpPeer<'_>,
) -> Result<TcpRelayStream, EngineError> {
    let tls_profile = peer.resume.tls_profile(tls_server_name);
    open_trojan_udp_tls_relay_stream(
        stream,
        TrojanUdpTlsOptions {
            profile: tls_profile,
            source_dir: proxy.config.source_dir(),
            server: peer.endpoint.server,
        },
    )
    .await
}
