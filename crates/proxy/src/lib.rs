#![cfg_attr(
    not(any(
        feature = "inbound-socks5",
        feature = "inbound-http-connect",
        feature = "inbound-vless",
    )),
    allow(dead_code, unused_imports, unused_variables, unreachable_code)
)]

mod groups;
mod inbound;
mod inventory;
mod logging;
mod outbound;
mod runtime;
mod transport;

pub use inventory::ProtocolInventory;
pub use runtime::{Proxy, RunningProxy};
