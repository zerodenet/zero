use zero_config::{InboundConfig, InboundProtocolConfig};
use zero_engine::EngineError;

use crate::adapters::vless::VlessAdapter;
use crate::protocol_registry::BoundInbound;
use crate::runtime::Proxy;
use crate::transport::QuicInbound;

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
            let InboundProtocolConfig::Vless {
                users,
                reality,
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
            let profile = vless::inbound_profile_from_config_users(users.iter().map(|user| {
                (
                    user.id.as_str(),
                    user.flow.as_deref(),
                    user.credential_id.as_deref(),
                    user.principal_key.as_deref(),
                    user.up_bps,
                    user.down_bps,
                )
            }))
            .map_err(EngineError::from)?;
            let reality = reality.as_ref().map(|reality| {
                vless::VlessRealityServerProfile::from_config_server(
                    reality.private_key.clone(),
                    reality.short_ids.clone(),
                    reality.server_name.clone(),
                    reality.cipher_suites.clone(),
                )
            });
            let ws = ws.clone();
            let grpc = grpc.clone();
            let h2 = h2.clone();
            let http_upgrade = http_upgrade.clone();
            let split_http = split_http.clone();
            let fallback = fallback.clone();
            let tls_acceptor = tls
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
                    ws,
                    grpc,
                    h2,
                    http_upgrade,
                    split_http,
                    fallback,
                },
                bound,
                shutdown_rx,
            )
            .await
        });
    }
}
