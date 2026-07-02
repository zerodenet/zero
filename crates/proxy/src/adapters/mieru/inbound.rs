use zero_config::{InboundConfig, InboundProtocolConfig};
use zero_engine::EngineError;

use crate::adapters::mieru::MieruAdapter;
use crate::protocol_registry::BoundInbound;
use crate::runtime::Proxy;

mod listener;

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
            let profile = match &inbound.protocol {
                InboundProtocolConfig::Mieru { users } => mieru::inbound_profile_from_config_users(
                    users
                        .iter()
                        .map(|user| (user.username.as_str(), user.password.as_str())),
                ),
                _ => {
                    return Err(EngineError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "mieru adapter received non-mieru inbound config",
                    )));
                }
            };
            listener::run_mieru_listener_with_bound(
                &p,
                listener::MieruInboundRequest { inbound, profile },
                bound.into_tcp(),
                shutdown_rx,
            )
            .await
        });
    }
}
