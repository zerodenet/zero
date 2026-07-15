mod listener;

use ::socks5::transport::Socks5InboundAcceptor;

use crate::runtime::inbound_operation::{InboundConnectionContext, TcpInboundListenerOperation};
use crate::transport::{MeteredStream, TcpRelayStream};

#[cfg(feature = "mixed")]
pub(crate) use listener::handle_socks5_connection;

pub(super) fn prepare(
    acceptor: Socks5InboundAcceptor,
) -> Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation> {
    Box::new(TcpInboundListenerOperation {
        protocol_name: "socks5",
        error_protocol_name: "socks5",
        request: acceptor,
        dispatch: |acceptor: Socks5InboundAcceptor, socket, context: InboundConnectionContext| async move {
            listener::handle_socks5_connection(
                context,
                MeteredStream::new(TcpRelayStream::from(socket)),
                &acceptor,
            )
            .await
        },
    })
}
