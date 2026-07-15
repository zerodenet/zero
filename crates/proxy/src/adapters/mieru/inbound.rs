//! Mieru inbound preparation and protocol handshake handoff.

use ::mieru::transport::MieruInboundListenerRequest;

use crate::runtime::inbound_operation::{InboundConnectionContext, TcpInboundListenerOperation};
use crate::transport::{MeteredStream, TcpRelayStream};

pub(super) fn prepare(
    profile: MieruInboundListenerRequest,
) -> Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation> {
    Box::new(TcpInboundListenerOperation {
        protocol_name: "mieru",
        error_protocol_name: "mieru",
        request: profile,
        dispatch: |profile: MieruInboundListenerRequest,
                   socket,
                   context: InboundConnectionContext| async move {
            let tcp_context = context.clone();
            let udp_context = context;
            let response = profile.response_protocol();
            match profile
                .accept_client(MeteredStream::new(TcpRelayStream::from(socket)))
                .await?
            {
                ::mieru::inbound::MieruInboundAcceptedSession::Tcp { session, stream } => {
                    tcp_context
                        .serve_with_client_response(session, stream, response)
                        .await
                }
                ::mieru::inbound::MieruInboundAcceptedSession::Udp { session, relay } => {
                    udp_context
                        .run_stream_udp_relay(session, relay, "mieru_udp")
                        .await
                }
            }
        },
    })
}
