use crate::runtime::udp_flow::managed::{
    datagram_manager::managed_datagram_socket_handler_box, ManagedDatagramFlowHandler,
};

pub(super) fn handler() -> Box<dyn ManagedDatagramFlowHandler> {
    managed_datagram_socket_handler_box::<
        zero_transport::shadowsocks_transport::ShadowsocksManagedDatagramFlowResume,
    >()
}
