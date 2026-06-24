//! Protocol adapter registry - eliminates per-protocol match arms in the proxy.
//!
//! Each protocol provides a `ProtocolAdapter` that knows its name, feature gate,
//! and how to validate its configuration. The `ProtocolRegistry` collects
//! adapters at startup and replaces the hard-coded match statements in
//! `ProtocolInventory`.

mod adapter;
mod defaults;
mod model;
mod registry;

pub(crate) use adapter::ProtocolAdapter;
pub(crate) use model::{BoundInbound, OutboundLeafRuntime};
pub(crate) use registry::ProtocolRegistry;
