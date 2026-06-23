use zero_config::{InboundConfig, InboundProtocolConfig};
use zero_engine::EngineError;

use crate::adapters::mieru::MieruAdapter;
use crate::protocol_adapter::BoundInbound;
use crate::runtime::Proxy;

impl MieruAdapter {
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
            let users = match &inbound.protocol {
                InboundProtocolConfig::Mieru { users } => users
                    .iter()
                    .map(|user| (user.username.clone(), user.password.clone()))
                    .collect::<Vec<_>>(),
                _ => {
                    return Err(EngineError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "mieru adapter received non-mieru inbound config",
                    )));
                }
            };
            crate::inbound::run_mieru_listener_with_bound(
                &p,
                crate::inbound::MieruInboundRequest { inbound, users },
                bound.into_tcp(),
                shutdown_rx,
            )
            .await
        });
    }
}
