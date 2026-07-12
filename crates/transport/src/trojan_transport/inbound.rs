use std::path::Path;

use zero_config::InboundProtocolConfig;
use zero_engine::EngineError;
use zero_platform_tokio::TokioSocket;

use crate::inbound_route::OpaqueStreamRoute;

type TrojanInboundTlsStream = crate::tls::InboundTlsStream<TokioSocket>;

#[derive(Clone)]
pub struct TrojanInboundListenerRequest {
    profile: trojan::inbound::TrojanInboundProfile,
    tls_acceptor: crate::tls::TlsAcceptor,
}

impl TrojanInboundListenerRequest {
    pub const ERROR_PROTOCOL_NAME: &'static str = "trojan";
    pub const UDP_PROTOCOL: &'static str = "trojan_udp";

    pub fn from_protocol_config(
        protocol: &InboundProtocolConfig,
        source_dir: Option<&Path>,
    ) -> Result<Self, EngineError> {
        let InboundProtocolConfig::Trojan { password, tls, .. } = protocol else {
            return Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "trojan inbound request received non-trojan inbound config",
            )));
        };

        Ok(Self {
            profile: trojan::inbound::TrojanInboundProfile::from_config_password(password.as_str()),
            tls_acceptor: crate::inbound_stack::build_required_tls_acceptor(
                source_dir,
                tls.as_ref(),
                "trojan requires TLS",
            )?,
        })
    }

    pub fn protocol_name(&self) -> &'static str {
        "trojan"
    }

    pub fn error_protocol_name(&self) -> &'static str {
        Self::ERROR_PROTOCOL_NAME
    }

    pub fn no_client_stream_route_defaults(
        &self,
    ) -> crate::inbound_route::NoClientStreamRouteDefaults {
        crate::inbound_route::NoClientStreamRouteDefaults {
            udp_protocol: Self::UDP_PROTOCOL,
        }
    }

    pub async fn accept_route(
        self,
        socket: TokioSocket,
    ) -> Result<
        OpaqueStreamRoute<trojan::inbound::TrojanInboundAcceptedSession<TrojanInboundTlsStream>>,
        EngineError,
    > {
        let stream =
            crate::inbound_stack::accept_tls_inbound_stream(socket, &self.tls_acceptor).await?;
        self.profile
            .accept_route_owned(trojan::inbound::TrojanInbound, stream)
            .await
            .map(OpaqueStreamRoute::new)
            .map_err(EngineError::from)
    }
}
