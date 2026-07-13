//! SOCKS5 protocol handshake and accepted-route handoff.

use zero_engine::EngineError;

use crate::runtime::inbound_operation::InboundConnectionContext;
use crate::transport::{MeteredStream, TcpRelayStream};

pub(crate) async fn handle_socks5_connection(
    context: InboundConnectionContext,
    metered: MeteredStream<TcpRelayStream>,
    acceptor: &zero_transport::socks5_transport::OwnedSocks5InboundAcceptor,
) -> Result<(), EngineError> {
    let tcp_context = context.clone();
    let udp_context = context;
    acceptor
        .accept_and_dispatch_command(
            metered,
            |session, stream| {
                tcp_context.serve_with_client_response(session, stream, acceptor.clone())
            },
            |setup, stream| async move {
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
