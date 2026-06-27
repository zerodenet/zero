use zero_config::{InboundConfig, InboundProtocolConfig};
use zero_engine::EngineError;

use crate::adapters::vless::VlessAdapter;
use crate::protocol_registry::BoundInbound;
use crate::runtime::Proxy;
use crate::transport::QuicInbound;

fn parse_inbound_users(
    inbound: &InboundConfig,
) -> Result<std::sync::Arc<[crate::inbound::vless::ConfiguredVlessUser]>, EngineError> {
    inbound
        .protocol
        .vless_users()
        .iter()
        .map(|user| {
            vless::VlessConfiguredUser::from_config(
                &user.id,
                user.flow.as_deref(),
                user.credential_id.clone(),
                user.principal_key.clone(),
                user.up_bps,
                user.down_bps,
            )
            .map(|user| crate::inbound::vless::ConfiguredVlessUser {
                id: user.id,
                user: user.user,
            })
            .map_err(EngineError::from)
        })
        .collect::<Result<Vec<_>, EngineError>>()
        .map(Into::into)
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
            let users = parse_inbound_users(&inbound)?;
            crate::inbound::run_vless_listener_with_bound(
                &p,
                crate::inbound::vless::model::VlessInboundRequest { inbound, users },
                bound,
                shutdown_rx,
            )
            .await
        });
    }
}
