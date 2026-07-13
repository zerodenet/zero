use std::io;
use std::path::Path;

use zero_config::{FallbackConfig, InboundProtocolConfig};
use zero_engine::EngineError;
use zero_platform_tokio::{ClientStream, TcpRelayStream, TokioSocket};

use crate::inbound_route::{OpaqueFallbackReplay, OpaqueMuxRoute, RouteAcceptResult};

mod bind;
mod carrier;
mod plan;

pub use bind::OwnedVlessInboundBindPlan;
use plan::{accept_vless_stream_route, OwnedVlessInboundTransportPlan};

fn record_client_stream<S>(stream: S) -> crate::MeteredStream<crate::RecordingStream<S>>
where
    S: ClientStream + 'static,
{
    crate::MeteredStream::new(crate::RecordingStream::new(stream))
}
#[derive(Clone)]
pub struct VlessInboundListenerRequest {
    profile: vless::inbound::VlessInboundProfile,
    transport: OwnedVlessInboundTransportPlan,
    fallback: Option<FallbackConfig>,
}

impl VlessInboundListenerRequest {
    pub const ERROR_PROTOCOL_NAME: &'static str = "vless";
    pub const UDP_PROTOCOL: &'static str = "vless_udp";
    pub const MUX_PROTOCOL: &'static str = "vless_mux";
    pub const PANIC_MESSAGE: &'static str = "vless mux task panicked";
    pub const ABORT_ON_END: bool = true;

    fn new(
        profile: vless::inbound::VlessInboundProfile,
        transport: OwnedVlessInboundTransportPlan,
        fallback: Option<FallbackConfig>,
    ) -> Self {
        Self {
            profile,
            transport,
            fallback,
        }
    }

    pub fn from_protocol_config(
        protocol: &InboundProtocolConfig,
        source_dir: Option<&Path>,
    ) -> Result<Self, EngineError> {
        let InboundProtocolConfig::Vless {
            users,
            reality,
            tls,
            ws,
            grpc,
            h2,
            http_upgrade,
            split_http,
            fallback,
            ..
        } = protocol
        else {
            return Err(EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "vless inbound request received non-vless inbound config",
            )));
        };

        let profile =
            vless::inbound::VlessInboundProfile::from_config_users(users.iter().map(|user| {
                (
                    user.id.as_str(),
                    user.flow.as_deref(),
                    user.credential_id.as_deref(),
                    user.principal_key.as_deref(),
                    user.up_bps,
                    user.down_bps,
                )
            }))
            .map_err(EngineError::from)?;

        let transport = OwnedVlessInboundTransportPlan::from_config_refs(
            source_dir,
            tls.as_deref(),
            reality.as_deref(),
            ws.as_deref(),
            grpc.as_deref(),
            h2.as_deref(),
            http_upgrade.as_deref(),
            split_http.as_deref(),
            fallback.as_deref(),
        )?;

        Ok(Self::new(profile, transport, fallback.as_deref().cloned()))
    }

    pub fn protocol_name(&self) -> &'static str {
        "vless"
    }

    pub fn error_protocol_name(&self) -> &'static str {
        Self::ERROR_PROTOCOL_NAME
    }

    pub fn recorded_mux_route_defaults(&self) -> crate::inbound_route::RecordedMuxRouteDefaults {
        crate::inbound_route::RecordedMuxRouteDefaults {
            udp_protocol: Self::UDP_PROTOCOL,
            mux_protocol: Self::MUX_PROTOCOL,
            panic_message: Self::PANIC_MESSAGE,
            abort_on_end: Self::ABORT_ON_END,
            udp_accept_log_message: Some("MUX stream accepted"),
        }
    }

    pub fn response_protocol(&self) -> vless::inbound::VlessInbound {
        vless::inbound::VlessInbound
    }

    async fn accept_tcp_route<S, FWrap>(
        self,
        socket: TokioSocket,
        wrap_stream: FWrap,
    ) -> Result<
        Option<
            RouteAcceptResult<
                OpaqueMuxRoute<vless::inbound::VlessAcceptedClientRoute<S>>,
                OpaqueFallbackReplay<TcpRelayStream>,
            >,
        >,
        EngineError,
    >
    where
        S: ClientStream + zero_core::InboundFallbackCapture<Stream = TcpRelayStream> + 'static,
        FWrap: Fn(TcpRelayStream) -> S + Clone + Send + 'static,
    {
        let Self {
            profile,
            transport,
            fallback,
        } = self;
        transport
            .accept_tcp_route(profile, fallback, socket, wrap_stream)
            .await
    }

    async fn accept_stream_route<T, S, FWrap>(
        self,
        stream: T,
        sni: Option<String>,
        wrap_stream: FWrap,
    ) -> Result<
        RouteAcceptResult<
            OpaqueMuxRoute<vless::inbound::VlessAcceptedClientRoute<S>>,
            OpaqueFallbackReplay<<S as zero_core::InboundFallbackCapture>::Stream>,
        >,
        EngineError,
    >
    where
        T: ClientStream + 'static,
        S: ClientStream + zero_core::InboundFallbackCapture + 'static,
        <S as zero_core::InboundFallbackCapture>::Stream: ClientStream + Send + 'static,
        FWrap: Fn(T) -> S + Clone + Send + 'static,
    {
        let Self {
            profile, fallback, ..
        } = self;
        accept_vless_stream_route(profile, fallback, stream, sni, wrap_stream).await
    }

    pub async fn accept_recorded_tcp_route(
        self,
        socket: TokioSocket,
    ) -> Result<
        Option<
            RouteAcceptResult<
                OpaqueMuxRoute<
                    vless::inbound::VlessAcceptedClientRoute<
                        crate::MeteredStream<crate::RecordingStream<TcpRelayStream>>,
                    >,
                >,
                OpaqueFallbackReplay<TcpRelayStream>,
            >,
        >,
        EngineError,
    > {
        self.accept_tcp_route(socket, record_client_stream).await
    }

    pub async fn accept_recorded_stream_route<T>(
        self,
        stream: T,
    ) -> Result<
        RouteAcceptResult<
            OpaqueMuxRoute<
                vless::inbound::VlessAcceptedClientRoute<
                    crate::MeteredStream<crate::RecordingStream<T>>,
                >,
            >,
            OpaqueFallbackReplay<T>,
        >,
        EngineError,
    >
    where
        T: ClientStream + Send + 'static,
    {
        self.accept_stream_route(stream, None, record_client_stream)
            .await
    }
}
