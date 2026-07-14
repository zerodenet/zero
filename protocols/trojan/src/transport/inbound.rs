use zero_platform_tokio::TokioSocket;
use zero_transport::inbound_route::{NoClientStreamRouteDefaults, OpaqueStreamRoute};
use zero_transport::tls::{InboundTlsStream, TlsAcceptor};
use zero_transport::RuntimeError;

type TrojanInboundTlsStream = InboundTlsStream<TokioSocket>;

#[derive(Clone)]
pub struct TrojanInboundListenerRequest {
    profile: crate::inbound::TrojanInboundProfile,
    tls_acceptor: TlsAcceptor,
}

impl TrojanInboundListenerRequest {
    pub const ERROR_PROTOCOL_NAME: &'static str = "trojan";
    pub const UDP_PROTOCOL: &'static str = "trojan_udp";

    pub fn new(profile: crate::inbound::TrojanInboundProfile, tls_acceptor: TlsAcceptor) -> Self {
        Self {
            profile,
            tls_acceptor,
        }
    }

    pub fn protocol_name(&self) -> &'static str {
        "trojan"
    }

    pub fn error_protocol_name(&self) -> &'static str {
        Self::ERROR_PROTOCOL_NAME
    }

    pub fn no_client_stream_route_defaults(&self) -> NoClientStreamRouteDefaults {
        NoClientStreamRouteDefaults {
            udp_protocol: Self::UDP_PROTOCOL,
        }
    }

    pub async fn accept_route(
        self,
        socket: TokioSocket,
    ) -> Result<
        OpaqueStreamRoute<crate::inbound::TrojanInboundAcceptedSession<TrojanInboundTlsStream>>,
        RuntimeError,
    > {
        let stream =
            zero_transport::inbound_stack::accept_tls_inbound_stream(socket, &self.tls_acceptor)
                .await?;
        self.profile
            .accept_route_owned(crate::inbound::TrojanInbound, stream)
            .await
            .map(OpaqueStreamRoute::new)
            .map_err(RuntimeError::from)
    }
}
