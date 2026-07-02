use tokio::io::AsyncWriteExt;
use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::unreachable_leaf;
use crate::adapters::trojan::TrojanAdapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::Proxy;
use crate::transport::{
    open_trojan_tls_stream, EstablishedTcpOutbound, MeteredStream, TcpOutboundFailure,
    TcpRelayStream, TrojanTlsOptions, TrojanTlsProfile,
};

impl TrojanAdapter {
    pub(super) async fn connect_tcp_impl(
        &self,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let ResolvedLeafOutbound::Trojan {
            tag,
            server,
            port,
            password,
            sni,
            insecure,
            client_fingerprint,
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf));
        };
        let config =
            trojan::tcp_connect_config_from_config(password, *sni, *insecure, *client_fingerprint);
        match connect_tcp(proxy, session, server, *port, config).await {
            Ok(upstream) => Ok(EstablishedTcpOutbound::proxied(
                *tag, *server, *port, upstream,
            )),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_upstream_trojan",
                error,
                upstream_endpoint: Some(((*server).to_string(), *port)),
            }),
        }
    }

    pub(super) async fn apply_relay_hop_impl(
        &self,
        proxy: &Proxy,
        stream: crate::transport::TcpRelayStream,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<crate::transport::TcpRelayStream, EngineError> {
        let ResolvedLeafOutbound::Trojan {
            password,
            sni,
            insecure,
            client_fingerprint,
            ..
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf).error);
        };
        let config =
            trojan::tcp_connect_config_from_config(password, *sni, *insecure, *client_fingerprint);
        apply_tcp_hop(proxy, stream, session, config).await
    }
}

async fn connect_tcp(
    proxy: &Proxy,
    session: &Session,
    server: &str,
    port: u16,
    config: trojan::TrojanTcpConnectConfig,
) -> Result<TcpRelayStream, EngineError> {
    let upstream = proxy
        .protocols
        .direct_connector()
        .connect_host(server, port, proxy.resolver.as_ref())
        .await?;
    let tls_stream = open_trojan_tls_stream(
        upstream,
        trojan_tls_options(proxy, server, config.tls_profile()),
    )
    .await?;
    let mut metered = MeteredStream::new(tls_stream);
    config.establish_tcp_tunnel(&mut metered, session).await?;
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

fn trojan_tls_options<'a>(
    proxy: &'a Proxy,
    server: &'a str,
    profile: trojan::TrojanTcpTlsProfile,
) -> TrojanTlsOptions<'a> {
    let (server_name, insecure, client_fingerprint) = profile.into_parts();
    TrojanTlsOptions {
        tls_profile: TrojanTlsProfile::from_parts(
            server_name.as_deref(),
            insecure,
            client_fingerprint.as_deref(),
        ),
        source_dir: proxy.config.source_dir(),
        server,
    }
}

async fn apply_tcp_hop(
    _proxy: &Proxy,
    mut stream: TcpRelayStream,
    session: &Session,
    config: trojan::TrojanTcpConnectConfig,
) -> Result<TcpRelayStream, EngineError> {
    config
        .establish_tcp_tunnel(&mut stream, session)
        .await
        .map_err(|error| EngineError::Io(std::io::Error::other(error)))?;
    Ok(stream)
}
