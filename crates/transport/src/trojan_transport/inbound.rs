use std::path::Path;

use zero_config::InboundProtocolConfig;
use zero_engine::EngineError;
use zero_platform_tokio::TokioSocket;

use crate::inbound_route::{
    OpaqueStreamRoute, ProtocolInboundRequestFactory, ProtocolInboundRequestMetadata,
    ProtocolStreamRouteDispatchMetadata, StreamRouteRequest,
};
type TrojanInboundTlsStream = crate::tls::InboundTlsStream<TokioSocket>;

#[derive(Clone)]
pub struct TrojanInboundListenerRequest {
    profile: trojan::inbound::TrojanInboundProfile,
    tls_acceptor: crate::tls::TlsAcceptor,
}

impl TrojanInboundListenerRequest {
    fn from_protocol_config(
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
}

impl ProtocolInboundRequestFactory for TrojanInboundListenerRequest {
    fn from_protocol_config(
        protocol: &InboundProtocolConfig,
        source_dir: Option<&Path>,
    ) -> Result<Self, EngineError> {
        TrojanInboundListenerRequest::from_protocol_config(protocol, source_dir)
    }
}

impl ProtocolInboundRequestMetadata for TrojanInboundListenerRequest {
    const ERROR_PROTOCOL_NAME: &'static str = "trojan";

    fn protocol_name(&self) -> &'static str {
        "trojan"
    }
}

impl ProtocolStreamRouteDispatchMetadata for TrojanInboundListenerRequest {
    const UDP_PROTOCOL: &'static str = "trojan_udp";
}

#[async_trait::async_trait]
impl StreamRouteRequest for TrojanInboundListenerRequest {
    type Route =
        OpaqueStreamRoute<trojan::inbound::TrojanInboundAcceptedSession<TrojanInboundTlsStream>>;

    async fn accept_route(self, socket: TokioSocket) -> Result<Self::Route, EngineError> {
        let stream =
            crate::inbound_stack::accept_tls_inbound_stream(socket, &self.tls_acceptor).await?;
        self.profile
            .accept_route_owned(trojan::inbound::TrojanInbound, stream)
            .await
            .map(OpaqueStreamRoute::new)
            .map_err(EngineError::from)
    }
}
