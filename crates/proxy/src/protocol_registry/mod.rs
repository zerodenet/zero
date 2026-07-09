//! Protocol registry - eliminates per-protocol match arms in the proxy.
//!
//! Each registered protocol contributes focused capability traits for support
//! metadata, inbound listeners, TCP outbound, UDP flows, and packet-path roles.
//! The `ProtocolRegistry` collects capability objects at startup and replaces
//! hard-coded match statements in `ProtocolInventory`.

mod capability;
mod context;
mod defaults;
mod model;
mod registry;

pub(crate) use capability::{
    InboundListenerCapability, ProtocolSupportCapability, RegisteredProtocolCapability,
    TcpOutboundCapability, UdpFlowCapability, UdpPacketPathCapability,
};
pub(crate) use context::{InboundAdapterContext, OutboundAdapterContext, UdpAdapterContext};
#[cfg(feature = "transport_quic")]
pub(crate) use defaults::bind_transport_inbound;
pub(crate) use model::{BoundInbound, OutboundLeafRuntime};
pub(crate) use registry::ProtocolRegistry;
