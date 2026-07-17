#[cfg(feature = "managed-datagram-runtime")]
mod flow;
#[cfg(feature = "managed-datagram-runtime")]
mod socket;

#[cfg(feature = "managed-datagram-runtime")]
pub(crate) use flow::{
    managed_datagram_handler_box, ManagedDatagramConnectorFlow, ManagedDatagramFlowConnector,
    ManagedDatagramResumeConnector,
};
#[cfg(feature = "managed-datagram-runtime")]
pub(crate) use socket::{
    managed_datagram_socket_handler_box, ManagedDatagramSocketConnectorFlow,
    ManagedDatagramSocketFlowConnector, ManagedDatagramSocketResumeConnector,
};
