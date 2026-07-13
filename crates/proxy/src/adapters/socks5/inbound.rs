mod listener;

use zero_config::InboundConfig;
use zero_engine::EngineError;

use crate::adapters::socks5::Socks5Adapter;

#[cfg(feature = "mixed")]
pub(crate) use listener::handle_socks5_connection;
pub(crate) use listener::run_socks5_listener_with_bound;

impl Socks5Adapter {
    pub(super) fn prepare_inbound_listener_impl(
        &self,
        inbound: InboundConfig,
    ) -> Result<
        Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation>,
        EngineError,
    > {
        let acceptor =
            zero_transport::socks5_transport::inbound_acceptor_from_protocol(&inbound.protocol)?;
        Ok(Box::new(
            crate::runtime::inbound_operation::InboundListenerOperation::new(
                move |proxy, bound: crate::protocol_registry::BoundInbound, shutdown_rx| async move {
                    run_socks5_listener_with_bound(
                        &proxy,
                        inbound,
                        acceptor,
                        bound.into_tcp(),
                        shutdown_rx,
                    )
                    .await
                },
            ),
        ))
    }
}
