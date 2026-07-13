#[cfg(feature = "shadowsocks")]
mod connection;
#[cfg(feature = "shadowsocks")]
mod response;
mod state;

#[cfg(feature = "shadowsocks")]
pub(crate) use connection::managed_datagram_connection_from_ops;
pub(in crate::runtime::udp_flow::managed) use state::ManagedDatagramState;
