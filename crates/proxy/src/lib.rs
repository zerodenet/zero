#![cfg_attr(
    not(any(feature = "socks5", feature = "http_connect", feature = "vless",)),
    allow(dead_code, unused_imports, unused_variables, unreachable_code)
)]

mod adapters;
mod groups;
mod inbound;
mod inventory;
mod logging;
mod process_lookup;
mod protocol_capability;
mod protocol_registry;
mod register;
mod runtime;
mod transport;

pub use inventory::ProtocolInventory;
pub use runtime::{Proxy, ProxyHandle, RunningProxy};
