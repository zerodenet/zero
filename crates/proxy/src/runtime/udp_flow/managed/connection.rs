mod model;
#[cfg(feature = "managed-stream-runtime")]
mod packet;
#[cfg(any(
    feature = "managed-stream-runtime",
    feature = "managed-datagram-runtime"
))]
mod response;
#[cfg(any(
    feature = "managed-stream-runtime",
    feature = "managed-datagram-runtime"
))]
mod tuple;

#[cfg(any(
    feature = "managed-stream-runtime",
    feature = "managed-datagram-runtime"
))]
pub(crate) use model::SharedManagedUdpConnection;
#[cfg(feature = "managed-datagram-runtime")]
pub(crate) use model::{ManagedDatagramUdpConnection, SharedManagedDatagramUdpConnection};
#[cfg(feature = "managed-stream-runtime")]
pub(crate) use packet::managed_packet_udp_connection_from_flow;
#[cfg(feature = "managed-stream-runtime")]
pub(crate) use packet::ManagedPacketUdpFlowConnection;
#[cfg(any(
    feature = "managed-stream-runtime",
    feature = "managed-datagram-runtime"
))]
pub(crate) use tuple::{managed_tuple_udp_connection_from_flow, ManagedTupleUdpFlowConnection};
