#[cfg(feature = "hysteria2")]
mod flow;
mod mismatch;
mod model;
#[cfg(feature = "shadowsocks")]
mod socket;

#[cfg(feature = "hysteria2")]
pub(crate) use model::ManagedDatagramFlowManager;
#[cfg(feature = "shadowsocks")]
pub(crate) use model::ManagedDatagramSocketFlowManager;
