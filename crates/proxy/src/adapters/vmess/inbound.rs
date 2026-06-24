use zero_config::{InboundConfig, InboundProtocolConfig};
use zero_engine::EngineError;

use crate::adapters::vmess::VmessAdapter;
use crate::protocol_adapter::BoundInbound;
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
            let users = users
                .iter()
                .map(|user| {
                    let id = vmess::parse_uuid(&user.id).map_err(|error| {
                        EngineError::Io(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            error,
                        ))
                    })?;
                    let cipher = vmess::VmessCipher::from_name(&user.cipher).ok_or_else(|| {
                        EngineError::Io(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            format!("vmess unknown cipher: {}", user.cipher),
                        ))
                    })?;
                    Ok(vmess::VmessUser {
                        id,
                        cipher,
                        credential_id: user.credential_id.clone(),
                        principal_key: user.principal_key.clone(),
                        up_bps: user.up_bps,
                        down_bps: user.down_bps,
                    })
                })
                .collect::<Result<Vec<_>, EngineError>>()?;
            crate::inbound::run_vmess_listener_with_bound(
                &p,
                crate::inbound::vmess::model::VmessInboundRequest {
                    inbound,
                    users,
                    tls,
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
