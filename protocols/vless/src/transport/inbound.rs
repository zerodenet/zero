use std::path::Path;

use zero_platform_tokio::{ClientStream, TcpRelayStream, TokioSocket};
use zero_traits::{
    GrpcTransportProfile, H2TransportProfile, HttpUpgradeTransportProfile, InboundFallbackProfile,
    ServerTlsProfile, SplitHttpTransportProfile, WebSocketTransportProfile,
};

use zero_transport::inbound_route::{OpaqueFallbackReplay, OpaqueMuxRoute, RouteAcceptResult};
use zero_transport::profile::OwnedInboundFallbackProfile;
use zero_transport::RuntimeError;

mod bind;
mod carrier;
mod plan;

pub use bind::OwnedVlessInboundBindPlan;
use plan::{accept_vless_stream_route, OwnedVlessInboundTransportPlan};

fn record_client_stream<S>(
    stream: S,
) -> zero_transport::MeteredStream<zero_transport::RecordingStream<S>>
where
    S: ClientStream + 'static,
{
    zero_transport::MeteredStream::new(zero_transport::RecordingStream::new(stream))
}
#[derive(Clone)]
pub struct OwnedVlessInboundListenerConfig {
    profile: crate::inbound::VlessInboundProfile,
    transport: OwnedVlessInboundTransportPlan,
    fallback: Option<OwnedInboundFallbackProfile>,
}

impl OwnedVlessInboundListenerConfig {
    #[allow(clippy::too_many_arguments)]
    pub fn from_config_refs<TTls, TWs, TGrpc, TH2, THttp, TSplit, TFallback>(
        source_dir: Option<&Path>,
        profile: crate::inbound::VlessInboundProfile,
        reality: Option<crate::reality::VlessRealityServerProfile>,
        tls: Option<&TTls>,
        ws: Option<&TWs>,
        grpc: Option<&TGrpc>,
        h2: Option<&TH2>,
        http_upgrade: Option<&THttp>,
        split_http: Option<&TSplit>,
        fallback: Option<&TFallback>,
    ) -> Result<Self, RuntimeError>
    where
        TTls: ServerTlsProfile + ?Sized,
        TWs: WebSocketTransportProfile + ?Sized,
        TGrpc: GrpcTransportProfile + ?Sized,
        TH2: H2TransportProfile + ?Sized,
        THttp: HttpUpgradeTransportProfile + ?Sized,
        TSplit: SplitHttpTransportProfile + ?Sized,
        TFallback: InboundFallbackProfile + ?Sized,
    {
        let transport = OwnedVlessInboundTransportPlan::from_profile_refs(
            source_dir,
            tls,
            reality,
            ws,
            grpc,
            h2,
            http_upgrade,
            split_http,
            fallback,
        )?;

        Ok(Self {
            profile,
            transport,
            fallback: fallback.map(OwnedInboundFallbackProfile::from_profile),
        })
    }
}

#[derive(Clone)]
pub struct VlessInboundListenerRequest {
    profile: crate::inbound::VlessInboundProfile,
    transport: OwnedVlessInboundTransportPlan,
    fallback: Option<OwnedInboundFallbackProfile>,
}

impl VlessInboundListenerRequest {
    pub const ERROR_PROTOCOL_NAME: &'static str = "vless";
    pub const UDP_PROTOCOL: &'static str = "vless_udp";
    pub const MUX_PROTOCOL: &'static str = "vless_mux";
    pub const PANIC_MESSAGE: &'static str = "vless mux task panicked";
    pub const ABORT_ON_END: bool = true;

    fn new(
        profile: crate::inbound::VlessInboundProfile,
        transport: OwnedVlessInboundTransportPlan,
        fallback: Option<OwnedInboundFallbackProfile>,
    ) -> Self {
        Self {
            profile,
            transport,
            fallback,
        }
    }

    pub fn protocol_name(&self) -> &'static str {
        "vless"
    }

    pub fn error_protocol_name(&self) -> &'static str {
        Self::ERROR_PROTOCOL_NAME
    }

    pub fn recorded_mux_route_defaults(
        &self,
    ) -> zero_transport::inbound_route::RecordedMuxRouteDefaults {
        zero_transport::inbound_route::RecordedMuxRouteDefaults {
            udp_protocol: Self::UDP_PROTOCOL,
            mux_protocol: Self::MUX_PROTOCOL,
            panic_message: Self::PANIC_MESSAGE,
            abort_on_end: Self::ABORT_ON_END,
            udp_accept_log_message: Some("MUX stream accepted"),
        }
    }

    pub fn response_protocol(&self) -> crate::inbound::VlessInbound {
        crate::inbound::VlessInbound
    }

    async fn accept_tcp_route<S, FWrap>(
        self,
        socket: TokioSocket,
        wrap_stream: FWrap,
    ) -> Result<
        Option<
            RouteAcceptResult<
                OpaqueMuxRoute<crate::inbound::VlessAcceptedClientRoute<S>>,
                OpaqueFallbackReplay<TcpRelayStream>,
            >,
        >,
        RuntimeError,
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
            OpaqueMuxRoute<crate::inbound::VlessAcceptedClientRoute<S>>,
            OpaqueFallbackReplay<<S as zero_core::InboundFallbackCapture>::Stream>,
        >,
        RuntimeError,
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
                    crate::inbound::VlessAcceptedClientRoute<
                        zero_transport::MeteredStream<
                            zero_transport::RecordingStream<TcpRelayStream>,
                        >,
                    >,
                >,
                OpaqueFallbackReplay<TcpRelayStream>,
            >,
        >,
        RuntimeError,
    > {
        self.accept_tcp_route(socket, record_client_stream).await
    }

    pub async fn accept_recorded_stream_route<T>(
        self,
        stream: T,
    ) -> Result<
        RouteAcceptResult<
            OpaqueMuxRoute<
                crate::inbound::VlessAcceptedClientRoute<
                    zero_transport::MeteredStream<zero_transport::RecordingStream<T>>,
                >,
            >,
            OpaqueFallbackReplay<T>,
        >,
        RuntimeError,
    >
    where
        T: ClientStream + Send + 'static,
    {
        self.accept_stream_route(stream, None, record_client_stream)
            .await
    }
}

impl From<OwnedVlessInboundListenerConfig> for VlessInboundListenerRequest {
    fn from(config: OwnedVlessInboundListenerConfig) -> Self {
        let OwnedVlessInboundListenerConfig {
            profile,
            transport,
            fallback,
        } = config;
        Self::new(profile, transport, fallback)
    }
}
