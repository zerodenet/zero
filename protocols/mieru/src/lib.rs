// Mieru protocol — lib.rs
//
// Implements the mieru proxy protocol (https://github.com/enfein/mieru).
// XChaCha20-Poly1305 AEAD, time-based key derivation, session lifecycle,
// TCP + UDP transport with random padding anti-detection.

#![cfg_attr(not(feature = "crypto"), no_std)]
#![allow(async_fn_in_trait)]

extern crate alloc;

pub mod metadata;
pub mod protocol;
#[cfg(feature = "crypto")]
pub mod segment;
#[cfg(feature = "crypto")]
pub mod session;
pub mod udp;

#[cfg(feature = "crypto")]
pub mod crypto;

#[cfg(feature = "crypto")]
mod inbound;
#[cfg(feature = "crypto")]
mod outbound;

#[cfg(feature = "crypto")]
pub use crypto::{
    derive_key, try_derive_keys, MieruCipher, NonceConfig, NoncePattern, USER_HINT_LEN,
};

#[cfg(feature = "crypto")]
pub use inbound::{MieruAccept, MieruInbound, MieruInboundDataCodec};
pub use metadata::{
    DataMetadata, SessionMetadata, ACK_CLIENT_TO_SERVER, ACK_SERVER_TO_CLIENT,
    CLOSE_SESSION_REQUEST, CLOSE_SESSION_RESPONSE, DATA_CLIENT_TO_SERVER, DATA_SERVER_TO_CLIENT,
    METADATA_LEN, OPEN_SESSION_REQUEST, OPEN_SESSION_RESPONSE,
};
#[cfg(feature = "crypto")]
pub use outbound::{
    establish_udp_flow_with_resume, spawn_udp_flow, MieruOutbound, MieruTcpTarget,
    MieruUdpFlowConnection, MieruUdpFlowHandle, MieruUdpFlowIo, MieruUdpFlowPacket,
    MieruUdpFlowResponse, MieruUdpFlowResponseReceiver, MieruUdpFlowSession,
};
pub use protocol::MieruProtocol;
#[cfg(feature = "crypto")]
pub use segment::{
    build_data_segment, build_session_segment, parse_segment, Segment, MAX_FRAGMENT,
};
#[cfg(feature = "crypto")]
pub use session::MieruSession;
#[cfg(feature = "crypto")]
pub use udp::MieruInboundUdpSession;
#[cfg(feature = "crypto")]
pub use udp::MieruUdpFlowSessions;
pub use udp::{
    connector_flow_from_resume, udp_flow_resume_from_config, MieruInboundUdpPacket,
    MieruInboundUdpRequest, MieruUdpAssociatePacket, MieruUdpAssociatePayload,
    MieruUdpConnectorFlow, MieruUdpFlowCodec, MieruUdpFlowConfig, MieruUdpFlowResume,
    MieruUdpFlowStore,
};
