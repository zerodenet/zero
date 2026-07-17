//! Prepared TCP operation contracts plus focused executor modules.
//!
//! The root stays as a facade so direct, socket, session, and transport-leaf
//! execution do not regrow into one large implementation bucket.

mod contract;
mod direct;
#[cfg(feature = "udp-runtime")]
mod session;
#[cfg(feature = "udp-runtime")]
mod socket;
#[cfg(feature = "udp-runtime")]
mod transport;

pub(crate) use contract::{PreparedTcpConnectOperation, PreparedTcpRelayOperation};
pub(crate) use direct::DirectTcpConnectOperation;
#[cfg(feature = "udp-runtime")]
pub(crate) use session::SessionTcpConnectOperation;
#[cfg(feature = "udp-runtime")]
pub(crate) use socket::{SocketTcpConnectOperation, SocketTcpRelayOperation};
#[cfg(feature = "udp-runtime")]
pub(crate) use transport::{TransportLeafTcpConnectOperation, TransportLeafTcpRelayOperation};
