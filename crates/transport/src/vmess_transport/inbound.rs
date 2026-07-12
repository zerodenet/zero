use std::{io, path::Path};

use zero_config::{GrpcConfig, InboundProtocolConfig, WebSocketConfig};
use zero_engine::EngineError;
use zero_platform_tokio::{TcpRelayStream, TokioSocket};

use crate::inbound_route::OpaqueMuxRoute;
use crate::inbound_stack::InboundStreamStack;
use crate::tls;

#[derive(Clone)]
pub struct VmessInboundListenerRequest {
    profile: vmess::inbound::VmessInboundProfile,
    tls_acceptor: tls::TlsAcceptor,
    ws: Option<WebSocketConfig>,
    grpc: Option<GrpcConfig>,
    protocol_name: &'static str,
}

impl VmessInboundListenerRequest {
    pub const ERROR_PROTOCOL_NAME: &'static str = "vmess";
    pub const UDP_PROTOCOL: &'static str = "vmess_udp";
    pub const MUX_PROTOCOL: &'static str = "vmess_mux";
    pub const PANIC_MESSAGE: &'static str = "vmess mux task panicked";
    pub const ABORT_ON_END: bool = false;
    pub const READ_ERROR_LOG: &'static str = "vmess mux frame read failed";

    pub fn from_protocol_config(
        protocol: &InboundProtocolConfig,
        source_dir: Option<&Path>,
    ) -> Result<Self, EngineError> {
        let InboundProtocolConfig::Vmess {
            users,
            tls,
            ws,
            grpc,
        } = protocol
        else {
            return Err(EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "vmess inbound request received non-vmess inbound config",
            )));
        };

        let profile =
            vmess::inbound::VmessInboundProfile::from_config_users(users.iter().map(|user| {
                (
                    user.id.as_str(),
                    user.cipher.as_str(),
                    user.credential_id.as_deref(),
                    user.principal_key.as_deref(),
                    user.up_bps,
                    user.down_bps,
                )
            }))
            .map_err(|error| EngineError::Io(io::Error::new(io::ErrorKind::InvalidInput, error)))?;

        let protocol_name = match (ws.as_deref(), grpc.as_deref()) {
            (Some(_), Some(_)) => {
                return Err(EngineError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "vmess: ws and grpc are mutually exclusive",
                )));
            }
            (Some(_), None) => "vmess+ws",
            (None, Some(_)) => "vmess+grpc",
            (None, None) => "vmess",
        };

        Ok(Self {
            profile,
            tls_acceptor: crate::inbound_stack::build_required_tls_acceptor(
                source_dir,
                tls.as_deref(),
                "vmess requires TLS",
            )?,
            ws: ws.as_deref().cloned(),
            grpc: grpc.as_deref().cloned(),
            protocol_name,
        })
    }

    pub fn protocol_name(&self) -> &'static str {
        self.protocol_name
    }

    pub fn error_protocol_name(&self) -> &'static str {
        Self::ERROR_PROTOCOL_NAME
    }

    pub fn no_client_mux_route_defaults(&self) -> crate::inbound_route::NoClientMuxRouteDefaults {
        crate::inbound_route::NoClientMuxRouteDefaults {
            udp_protocol: Self::UDP_PROTOCOL,
            mux_protocol: Self::MUX_PROTOCOL,
            panic_message: Self::PANIC_MESSAGE,
            abort_on_end: Self::ABORT_ON_END,
            read_error_log: Self::READ_ERROR_LOG,
        }
    }

    pub async fn accept_route(
        self,
        socket: TokioSocket,
    ) -> Result<
        OpaqueMuxRoute<
            vmess::mux::VmessInboundAcceptedStream<vmess::stream::VmessAeadStream<TcpRelayStream>>,
        >,
        EngineError,
    > {
        let stream = crate::inbound_stack::accept_tls_inbound_stream_stack(
            socket,
            &self.tls_acceptor,
            InboundStreamStack {
                ws: self.ws.as_ref(),
                grpc: self.grpc.as_ref(),
                h2: None,
            },
            "vmess: ws and grpc are mutually exclusive",
        )
        .await?;
        self.profile
            .accept_route_owned(vmess::inbound::VmessInbound, stream)
            .await
            .map(OpaqueMuxRoute::new)
            .map_err(EngineError::from)
    }
}
