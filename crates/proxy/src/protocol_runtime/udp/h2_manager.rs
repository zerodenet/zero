#[cfg(feature = "hysteria2")]
use std::collections::HashMap;

#[cfg(feature = "hysteria2")]
mod bridge;
#[cfg(feature = "hysteria2")]
mod codec;
#[cfg(feature = "hysteria2")]
mod establish;
#[cfg(feature = "hysteria2")]
mod model;
#[cfg(feature = "hysteria2")]
mod send;
#[cfg(feature = "hysteria2")]
mod stream;

#[cfg(feature = "hysteria2")]
pub(crate) use model::H2SendExisting;

#[cfg(feature = "hysteria2")]
use model::{H2Entry, H2Key};

#[cfg(feature = "hysteria2")]
pub(crate) struct H2ChainManager {
    upstreams: HashMap<H2Key, H2Entry>,
}

#[cfg(feature = "hysteria2")]
impl H2ChainManager {
    pub(crate) fn new() -> Self {
        Self {
            upstreams: HashMap::new(),
        }
    }
}
