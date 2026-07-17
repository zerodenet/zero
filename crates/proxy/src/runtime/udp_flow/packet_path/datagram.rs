mod access;
mod build;
mod model;

#[cfg(feature = "managed-datagram-runtime")]
pub(crate) use build::{
    udp_datagram_source, udp_datagram_source_from_build, UdpDatagramSourceBuild,
};
pub(crate) use model::{
    DatagramCodec, UdpDatagramDescriptor, UdpDatagramEndpoint, UdpDatagramKey, UdpDatagramSource,
};
