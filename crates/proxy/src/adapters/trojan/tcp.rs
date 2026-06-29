use tokio::io::AsyncWriteExt;
use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::unreachable_leaf;
use crate::adapters::trojan::TrojanAdapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::Proxy;
use crate::transport::{
    open_trojan_udp_tls_stream, EstablishedTcpOutbound, MeteredStream, TcpOutboundFailure,
    TcpRelayStream, TrojanTlsProfile, TrojanUdpTlsOptions,
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
        match connect_tcp(TrojanTcpConnect {
            proxy,
            session,
            server,
            port: *port,
            password,
            sni: *sni,
            insecure: *insecure,
            client_fingerprint: *client_fingerprint,
        })
        .await
        {
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
        let ResolvedLeafOutbound::Trojan { password, .. } = leaf else {
            return Err(unreachable_leaf(self.name(), leaf).error);
        };
        apply_tcp_hop(proxy, stream, session, password).await
    }
}

struct TrojanTcpConnect<'a> {
    proxy: &'a Proxy,
    session: &'a Session,
    server: &'a str,
    port: u16,
    password: &'a str,
    sni: Option<&'a str>,
    insecure: bool,
    client_fingerprint: Option<&'a str>,
}

async fn connect_tcp(request: TrojanTcpConnect<'_>) -> Result<TcpRelayStream, EngineError> {
    let TrojanTcpConnect {
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
            trojan::TrojanTcpTlsProfile::from_config_parts(sni, insecure, client_fingerprint),
        ),
    )
    .await?;
    let mut metered = MeteredStream::new(tls_stream);
    let profile = trojan::TrojanTcpOutboundProfile::from_config_password(password);
    profile.establish_tcp_tunnel(&mut metered, session).await?;
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
) -> TrojanUdpTlsOptions<'a> {
    TrojanUdpTlsOptions {
        tls_profile: TrojanTlsProfile::from_parts(
            profile.server_name(),
            profile.insecure(),
            profile.client_fingerprint(),
        ),
        source_dir: proxy.config.source_dir(),
        server,
    }
}

async fn apply_tcp_hop(
    _proxy: &Proxy,
    mut stream: TcpRelayStream,
    session: &Session,
    password: &str,
) -> Result<TcpRelayStream, EngineError> {
    let profile = trojan::TrojanTcpOutboundProfile::from_config_password(password);
    profile
        .establish_tcp_tunnel(&mut stream, session)
        .await
        .map_err(|error| EngineError::Io(std::io::Error::other(error)))?;
    Ok(stream)
}
