mod connector;
mod manager;

#[cfg(feature = "hysteria2")]
pub(crate) use connector::managed_datagram_handler_box;
#[cfg(feature = "shadowsocks")]
pub(crate) use connector::managed_datagram_socket_handler_box;
