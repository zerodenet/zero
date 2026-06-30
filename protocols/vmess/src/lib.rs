#![allow(async_fn_in_trait)]

mod crypto;
mod inbound;
mod metadata;
pub mod mux;
mod outbound;
mod shared;
mod stream;
pub mod udp;

pub use inbound::{
    inbound_profile_from_config_users, IntoVmessInboundUserConfig, VmessAccept, VmessInbound,
    VmessInboundProfile, VmessInboundUserConfigParts, VmessUser,
};
pub use metadata::VmessProtocol;
pub use outbound::{
    establish_tcp_outbound_session, establish_tcp_outbound_stream, tcp_connect_config_from_config,
    wrap_tcp_outbound_stream, VmessOutbound, VmessOutboundSession, VmessTcpConnectConfig,
    VmessTcpSessionTarget,
};
pub use shared::{
    parse_uuid, VmessCipher, AUTH_ID_LEN, CMD_TCP, CMD_UDP, GCM_TAG_LEN, MUX_COOL_DOMAIN,
    MUX_COOL_PORT, VERSION,
};
pub use stream::{wrap_tcp_inbound_stream, VmessAeadStream};
pub use udp::{
    VmessInboundMuxUdpResponder, VmessInboundUdpClientResponse, VmessInboundUdpResponder,
};
