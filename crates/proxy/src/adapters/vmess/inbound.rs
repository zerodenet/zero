use zero_config::{InboundConfig, InboundProtocolConfig};
use zero_engine::EngineError;

use crate::adapters::vmess::VmessAdapter;
use crate::protocol_registry::BoundInbound;
use crate::runtime::Proxy;

impl VmessAdapter {
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
            let (users, tls, ws, grpc) = match &inbound.protocol {
                InboundProtocolConfig::Vmess {
                    users,
                    tls,
                    ws,
                    grpc,
                } => (users.clone(), tls.clone(), ws.clone(), grpc.clone()),
                _ => {
                    return Err(EngineError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "vmess adapter received non-vmess inbound config",
                    )));
                }
            };
            let tls_cfg = tls.ok_or_else(|| {
                EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "vmess requires TLS",
                ))
            })?;
            let tls_acceptor =
                crate::transport::build_tls_acceptor(&tls_cfg, p.config.source_dir())?;
            let profile = vmess::VmessInboundProfile::from_config_users(users.iter().map(|user| {
                (
                    user.id.as_str(),
                    user.cipher.as_str(),
                    user.credential_id.as_deref(),
                    user.principal_key.as_deref(),
                    user.up_bps,
                    user.down_bps,
                )
            }))
            .map_err(|error| {
                EngineError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, error))
            })?;
            crate::inbound::run_vmess_listener_with_bound(
                &p,
                crate::inbound::vmess::model::VmessInboundRequest {
                    inbound,
                    profile,
                    tls_acceptor,
                    ws,
                    grpc,
                },
                bound.into_tcp(),
                shutdown_rx,
            )
            .await
        });
    }
}
