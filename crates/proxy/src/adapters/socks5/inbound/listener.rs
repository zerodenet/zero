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
    match acceptor.accept_command(&mut metered).await? {
        ::socks5::Socks5Request::Connect(session) => {
            tcp_context
                .serve_with_client_response(*session, metered, acceptor.clone())
                .await
        }
        ::socks5::Socks5Request::UdpAssociate(request) => {
            let setup = setup_inbound_udp_association(&mut metered, request).await?;
            udp_context
                .run_udp_association(
                    metered,
                    setup.relay,
                    setup.pending_control_traffic,
                    setup.handler,
                )
                .await
        }
    }
}
