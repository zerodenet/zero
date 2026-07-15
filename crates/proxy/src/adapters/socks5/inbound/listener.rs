//! SOCKS5 protocol handshake and accepted-route handoff.

use ::socks5::transport::{setup_inbound_udp_association, Socks5InboundAcceptor};
use zero_engine::EngineError;

use crate::runtime::inbound_operation::InboundConnectionContext;
use crate::transport::{MeteredStream, TcpRelayStream};

pub(crate) async fn handle_socks5_connection(
    context: InboundConnectionContext,
    metered: MeteredStream<TcpRelayStream>,
    acceptor: &Socks5InboundAcceptor,
) -> Result<(), EngineError> {
    let tcp_context = context.clone();
    let udp_context = context;
    let mut metered = metered;
    acceptor
        .accept_command(&mut metered)
        .await?
        .dispatch_with_handlers(
            metered,
            |session, stream| {
                tcp_context.serve_with_client_response(session, stream, acceptor.clone())
            },
            |request, mut stream| async move {
                let setup = setup_inbound_udp_association(&mut stream, request).await?;
                udp_context
                    .run_udp_association(
                        stream,
                        setup.relay,
                        setup.pending_control_traffic,
                        setup.handler,
                    )
                    .await
            },
        )
        .await
}
