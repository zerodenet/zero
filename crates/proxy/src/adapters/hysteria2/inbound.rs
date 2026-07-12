mod transport;

use zero_config::InboundConfig;
use zero_engine::EngineError;

use crate::adapters::hysteria2::Hysteria2Adapter;
use crate::protocol_registry::BoundInbound;
use crate::runtime::Proxy;

pub(crate) use transport::run_hysteria2_listener_with_bound;

impl Hysteria2Adapter {
    pub(super) fn spawn_inbound_impl(
        &self,
        proxy: &Proxy,
        inbound: InboundConfig,
        bound: BoundInbound,
        shutdown_rx: tokio::sync::watch::Receiver<bool>,
        listeners: &mut tokio::task::JoinSet<Result<(), EngineError>>,
    ) {
        let proxy = proxy.clone();
        listeners.spawn(async move {
            let profile =
                zero_transport::hysteria2_quic::inbound_profile_from_protocol(&inbound.protocol)?;
            run_hysteria2_listener_with_bound(&proxy, inbound, profile, bound, shutdown_rx).await
        });
    }
}
