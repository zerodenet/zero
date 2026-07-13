//! Mieru inbound preparation and protocol handshake handoff.

use zero_engine::EngineError;

use crate::runtime::inbound_operation::{InboundConnectionContext, TcpInboundListenerOperation};
use crate::transport::{MeteredStream, TcpRelayStream};

impl crate::adapters::mieru::MieruAdapter {
    pub(super) fn prepare_inbound_listener_impl(
        &self,
        inbound: zero_config::InboundConfig,
    ) -> Result<
        Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation>,
        EngineError,
    > {
        let profile =
            zero_transport::mieru_transport::inbound_profile_from_protocol(&inbound.protocol)?;
        Ok(Box::new(TcpInboundListenerOperation {
            inbound_tag: inbound.tag,
            protocol_name: "mieru",
            error_protocol_name: "mieru",
            request: profile,
            dispatch: |profile: zero_transport::mieru_transport::OwnedMieruInboundProfile,
                       socket,
                       context: InboundConnectionContext| async move {
                let tcp_context = context.clone();
                let udp_context = context;
                let response = profile.response_protocol();
                profile
                    .accept_and_dispatch_client(
                        MeteredStream::new(TcpRelayStream::from(socket)),
                        move |session, stream| {
                            tcp_context.serve_with_client_response(session, stream, response)
                        },
                        move |session, relay| {
                            udp_context.run_stream_udp_relay(session, relay, "mieru_udp")
                        },
                    )
                    .await
            },
        }))
    }
}
