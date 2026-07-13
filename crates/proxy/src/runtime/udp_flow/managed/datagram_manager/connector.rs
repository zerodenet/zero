#[cfg(feature = "hysteria2")]
mod flow;
#[cfg(feature = "shadowsocks")]
mod socket;

#[cfg(feature = "hysteria2")]
pub(crate) use flow::{managed_datagram_handler_box, ManagedDatagramFlowConnector};
#[cfg(feature = "shadowsocks")]
pub(crate) use socket::{managed_datagram_socket_handler_box, ManagedDatagramSocketFlowConnector};
