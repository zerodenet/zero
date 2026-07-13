mod listener;

use zero_config::InboundConfig;
use zero_engine::EngineError;

use crate::adapters::socks5::Socks5Adapter;
use crate::runtime::inbound_operation::{InboundConnectionContext, TcpInboundListenerOperation};
use crate::transport::{MeteredStream, TcpRelayStream};

#[cfg(feature = "mixed")]
pub(crate) use listener::handle_socks5_connection;

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
        Ok(Box::new(TcpInboundListenerOperation {
            inbound_tag: inbound.tag,
            protocol_name: "socks5",
            error_protocol_name: "socks5",
            request: acceptor,
            dispatch: |acceptor: zero_transport::socks5_transport::OwnedSocks5InboundAcceptor,
                       socket,
                       context: InboundConnectionContext| async move {
                listener::handle_socks5_connection(
                    context,
                    MeteredStream::new(TcpRelayStream::from(socket)),
                    &acceptor,
                )
                .await
            },
        }))
    }
}
