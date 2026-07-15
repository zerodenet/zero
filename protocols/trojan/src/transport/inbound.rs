use std::path::Path;

use zero_platform_tokio::TokioSocket;
use zero_traits::ServerTlsProfile;
use zero_transport::inbound_route::{NoClientStreamRouteDefaults, OpaqueStreamRoute};
use zero_transport::tls::{InboundTlsStream, TlsAcceptor};
use zero_transport::RuntimeError;

use super::options::TrojanInboundOptionsRef;

type TrojanInboundTlsStream = InboundTlsStream<TokioSocket>;

#[derive(Clone)]
struct OwnedTrojanInboundListenerConfig {
    profile: crate::inbound::TrojanInboundProfile,
    tls_acceptor: TlsAcceptor,
}

impl OwnedTrojanInboundListenerConfig {
    fn from_config_refs<TTls>(
        source_dir: Option<&Path>,
        profile: crate::inbound::TrojanInboundProfile,
        tls: Option<&TTls>,
    ) -> Result<Self, RuntimeError>
    where
        TTls: ServerTlsProfile + ?Sized,
    {
        Ok(Self {
            profile,
            tls_acceptor: zero_transport::inbound_stack::build_required_tls_acceptor(
                source_dir,
                tls,
                "trojan requires TLS",
            )?,
        })
    }
}

#[derive(Clone)]
pub struct TrojanInboundListenerRequest {
    profile: crate::inbound::TrojanInboundProfile,
    tls_acceptor: TlsAcceptor,
}

impl TrojanInboundListenerRequest {
    pub const ERROR_PROTOCOL_NAME: &'static str = "trojan";
    pub const UDP_PROTOCOL: &'static str = "trojan_udp";

    fn new(profile: crate::inbound::TrojanInboundProfile, tls_acceptor: TlsAcceptor) -> Self {
        Self {
            profile,
            tls_acceptor,
        }
    }

    pub fn from_config_refs<TTls>(
        source_dir: Option<&Path>,
        profile: crate::inbound::TrojanInboundProfile,
        tls: Option<&TTls>,
    ) -> Result<Self, RuntimeError>
    where
        TTls: ServerTlsProfile + ?Sized,
    {
        OwnedTrojanInboundListenerConfig::from_config_refs(source_dir, profile, tls).map(Into::into)
    }

    pub fn from_options_refs<TTls>(
        source_dir: Option<&Path>,
        options: TrojanInboundOptionsRef<'_>,
        tls: Option<&TTls>,
    ) -> Result<Self, RuntimeError>
    where
        TTls: ServerTlsProfile + ?Sized,
    {
        Self::from_config_refs(
            source_dir,
            crate::inbound::TrojanInboundProfile::from_config_password(options.password),
            tls,
        )
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

impl From<OwnedTrojanInboundListenerConfig> for TrojanInboundListenerRequest {
    fn from(config: OwnedTrojanInboundListenerConfig) -> Self {
        let OwnedTrojanInboundListenerConfig {
            profile,
            tls_acceptor,
        } = config;
        Self::new(profile, tls_acceptor)
    }
}
