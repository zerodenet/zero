mod handler;
mod model;
#[cfg(feature = "socks5")]
mod resume;
#[cfg(feature = "socks5")]
mod target;
#[cfg(feature = "socks5")]
mod transport;

pub(crate) use handler::UpstreamAssociationHandler;
pub(crate) use model::UpstreamAssociationSend;
#[cfg(feature = "udp-runtime")]
pub(crate) use model::UpstreamUdpHandlers;
#[cfg(feature = "socks5")]
pub(crate) use model::{UpstreamAssociationCloseReason, UpstreamAssociationStages};
#[cfg(feature = "socks5")]
pub(crate) use resume::handles_registered_resume;
#[cfg(feature = "socks5")]
pub(crate) use target::UpstreamAssociationTarget;
#[cfg(feature = "socks5")]
pub(crate) use transport::UpstreamAssociationTransport;
