use zero_config::{
    FallbackConfig, GrpcConfig, H2Config, HttpUpgradeConfig, InboundConfig, InboundProtocolConfig,
    SplitHttpConfig, TlsConfig, WebSocketConfig,
};
use zero_engine::EngineError;

use crate::adapters::vless::VlessAdapter;
use crate::protocol_registry::BoundInbound;
use crate::runtime::Proxy;
use crate::transport::QuicInbound;

fn parse_inbound_profile(
    inbound: &InboundConfig,
) -> Result<vless::VlessInboundProfile, EngineError> {
    vless::VlessInboundProfile::from_config_users(inbound.protocol.vless_users().iter().map(
        |user| {
            (
                user.id.clone(),
                user.flow.clone(),
                user.credential_id.clone(),
                user.principal_key.clone(),
                user.up_bps,
                user.down_bps,
            )
        },
    ))
    .map_err(EngineError::from)
}

fn parse_reality_profile(inbound: &InboundConfig) -> Option<vless::VlessRealityServerProfile> {
    inbound.protocol.vless_reality().map(|reality| {
        vless::VlessRealityServerProfile::from_config_parts(
            reality.private_key.clone(),
            reality.short_ids.clone(),
            reality.server_name.clone(),
            reality.cipher_suites.clone(),
        )
    })
}

struct VlessInboundTransportConfig {
    tls: Option<Box<TlsConfig>>,
    ws: Option<Box<WebSocketConfig>>,
    grpc: Option<Box<GrpcConfig>>,
    h2: Option<Box<H2Config>>,
    http_upgrade: Option<Box<HttpUpgradeConfig>>,
    split_http: Option<Box<SplitHttpConfig>>,
    fallback: Option<Box<FallbackConfig>>,
}

fn parse_transport_config(
    inbound: &InboundConfig,
) -> Result<VlessInboundTransportConfig, EngineError> {
    let InboundProtocolConfig::Vless {
        tls,
        ws,
        grpc,
        h2,
        http_upgrade,
        split_http,
        fallback,
        ..
    } = &inbound.protocol
    else {
        return Err(EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "vless adapter received non-vless inbound config",
        )));
    };
    Ok(VlessInboundTransportConfig {
        tls: tls.clone(),
        ws: ws.clone(),
        grpc: grpc.clone(),
        h2: h2.clone(),
        http_upgrade: http_upgrade.clone(),
        split_http: split_http.clone(),
        fallback: fallback.clone(),
    })
}

impl VlessAdapter {
    pub(super) async fn bind_inbound_impl(
        &self,
        inbound: &InboundConfig,
        source_dir: Option<&std::path::Path>,
    ) -> Result<BoundInbound, EngineError> {
        let listen = format!("{}:{}", inbound.listen.address, inbound.listen.port);
        if let InboundProtocolConfig::Vless {
            quic: Some(ref quic),
            ..
        } = inbound.protocol
        {
            if let (Some(cert), Some(key)) = (&quic.cert_path, &quic.key_path) {
                let endpoint = QuicInbound::bind(&listen, cert, key, source_dir).await?;
                return Ok(BoundInbound::Quic(endpoint));
            }
        }
        let tcp = zero_platform_tokio::TokioListener::bind(&listen)
            .await
            .map_err(EngineError::Io)?;
        Ok(BoundInbound::Tcp(tcp))
    }

    pub(super) fn spawn_inbound_impl(
        &self,
        proxy: &Proxy,
        inbound: InboundConfig,
        bound: BoundInbound,
        shutdown_rx: tokio::sync::watch::Receiver<bool>,
        listeners: &mut tokio::task::JoinSet<Result<(), EngineError>>,
    ) {
        let p = proxy.clone();
        listeners.spawn(async move {
            let profile = parse_inbound_profile(&inbound)?;
            let reality = parse_reality_profile(&inbound);
            let transport = parse_transport_config(&inbound)?;
            let tls_acceptor = transport
                .tls
                .as_deref()
                .map(|tls| crate::transport::build_tls_acceptor(tls, p.config.source_dir()))
                .transpose()?;
            crate::inbound::run_vless_listener_with_bound(
                &p,
                crate::inbound::vless::model::VlessInboundRequest {
                    inbound,
                    profile,
                    reality,
                    tls_acceptor,
                    ws: transport.ws,
                    grpc: transport.grpc,
                    h2: transport.h2,
                    http_upgrade: transport.http_upgrade,
                    split_http: transport.split_http,
                    fallback: transport.fallback,
                },
                bound,
                shutdown_rx,
            )
            .await
        });
    }
}
