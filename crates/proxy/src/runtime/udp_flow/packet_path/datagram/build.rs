use std::sync::Arc;

use zero_core::Address;

use super::model::{DatagramCodec, UdpDatagramDescriptor, UdpDatagramSource};

#[cfg(feature = "managed-datagram-runtime")]
pub(crate) fn udp_datagram_source(
    tag: &str,
    server: &str,
    port: u16,
    cache_key: String,
    codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
) -> UdpDatagramSource {
    UdpDatagramSource {
        descriptor: UdpDatagramDescriptor {
            tag: tag.to_owned(),
            server: server.to_owned(),
            port,
            cache_key,
        },
        codec,
    }
}

#[cfg(feature = "managed-datagram-runtime")]
pub(crate) trait UdpDatagramSourceBuild {
    fn into_parts(
        self,
    ) -> (
        String,
        String,
        u16,
        String,
        Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
    );
}

#[cfg(feature = "managed-datagram-runtime")]
pub(crate) fn udp_datagram_source_from_build(
    build: impl UdpDatagramSourceBuild,
) -> UdpDatagramSource {
    let (tag, server, port, cache_key, codec) = build.into_parts();
    udp_datagram_source(&tag, &server, port, cache_key, codec)
}
