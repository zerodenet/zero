//! Mieru inbound preparation and protocol handshake handoff.

use zero_config::InboundProtocolConfig;
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
        let profile = match &inbound.protocol {
            InboundProtocolConfig::Mieru { users } => {
                ::mieru::transport::inbound_listener_request_from_users(
                    users
                        .iter()
                        .map(|user| (user.username.as_str(), user.password.as_str())),
                )
            }
            _ => {
                return Err(EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "mieru inbound listener received non-mieru inbound config",
                )));
            }
        };
        Ok(Box::new(TcpInboundListenerOperation {
            inbound_tag: inbound.tag,
            protocol_name: "mieru",
            error_protocol_name: "mieru",
            request: profile,
            dispatch: |profile: ::mieru::transport::MieruInboundListenerRequest,
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
