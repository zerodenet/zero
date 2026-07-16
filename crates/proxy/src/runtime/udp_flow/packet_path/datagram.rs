use std::sync::Arc;

use zero_core::Address;

/// Datagram codec for encoding/decoding inner protocol datagrams.
pub(crate) use zero_traits::DatagramCodec;

/// Datagram source params for a relay-chain final hop over a packet path.
///
/// Produced by `PreparedUdpPacketPathOperation::into_datagram_source`. The
/// `cache_key` feeds packet-path cache identity without exposing raw config
/// parsing to the manager.
#[derive(Clone)]
pub(crate) struct UdpDatagramDescriptor {
    pub(crate) tag: String,
    pub(crate) server: String,
    pub(crate) port: u16,
    pub(crate) cache_key: String,
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
impl UdpDatagramDescriptor {
    pub(crate) fn key_part(&self) -> UdpDatagramKey {
        UdpDatagramKey {
            tag: self.tag.clone(),
            server: self.server.clone(),
            port: self.port,
            cache_key: self.cache_key.clone(),
        }
    }

    pub(crate) fn endpoint(&self) -> UdpDatagramEndpoint {
        UdpDatagramEndpoint {
            server: self.server.clone(),
            port: self.port,
        }
    }
}

/// Adapter-provided datagram role output for packet-path relay chains.
///
/// The descriptor is the generic chain-management surface. The codec is the
/// protocol-provided packet framing object for the selected datagram hop.
#[derive(Clone)]
pub(crate) struct UdpDatagramSource {
    pub(crate) descriptor: UdpDatagramDescriptor,
    pub(crate) codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
}

#[cfg(feature = "shadowsocks")]
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

#[cfg(feature = "shadowsocks")]
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

#[cfg(feature = "shadowsocks")]
pub(crate) fn udp_datagram_source_from_build(
    build: impl UdpDatagramSourceBuild,
) -> UdpDatagramSource {
    let (tag, server, port, cache_key, codec) = build.into_parts();
    udp_datagram_source(&tag, &server, port, cache_key, codec)
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
impl UdpDatagramSource {
    pub(crate) fn descriptor(&self) -> &UdpDatagramDescriptor {
        &self.descriptor
    }
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct UdpDatagramKey {
    pub(crate) tag: String,
    pub(crate) server: String,
    pub(crate) port: u16,
    pub(crate) cache_key: String,
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct UdpDatagramEndpoint {
    server: String,
    port: u16,
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
impl UdpDatagramEndpoint {
    pub(crate) fn target(&self) -> Address {
        Address::Domain(self.server.clone())
    }

    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    pub(crate) fn upstream(&self) -> (String, u16) {
        (self.server.clone(), self.port)
    }
}
