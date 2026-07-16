#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use super::{
    PacketPathCarrierDescriptor, UdpDatagramDescriptor, UdpDatagramKey, UdpDatagramSource,
};

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PacketPathLookupKey {
    carrier_cache_key: String,
    datagram: UdpDatagramKey,
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
impl PacketPathLookupKey {
    pub(crate) fn from_parts(
        carrier: &PacketPathCarrierDescriptor,
        datagram: UdpDatagramKey,
    ) -> Self {
        Self {
            carrier_cache_key: carrier.cache_key.clone(),
            datagram,
        }
    }

    pub(crate) fn datagram_endpoint(&self) -> (String, u16) {
        (self.datagram.server.clone(), self.datagram.port)
    }

    pub(crate) fn into_path_parts(self) -> (String, UdpDatagramKey) {
        (self.carrier_cache_key, self.datagram)
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PacketPathFlowSnapshot {
    carrier_cache_key: String,
    datagram: UdpDatagramKey,
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
impl PacketPathFlowSnapshot {
    fn from_parts(datagram: &UdpDatagramDescriptor, carrier: &PacketPathCarrierDescriptor) -> Self {
        Self {
            carrier_cache_key: carrier.cache_key.clone(),
            datagram: datagram.key_part(),
        }
    }

    pub(crate) fn lookup_key(&self) -> PacketPathLookupKey {
        PacketPathLookupKey {
            carrier_cache_key: self.carrier_cache_key.clone(),
            datagram: self.datagram.clone(),
        }
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
pub(crate) struct PacketPathFlowBinding {
    datagram: UdpDatagramSource,
    flow_snapshot: PacketPathFlowSnapshot,
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
impl PacketPathFlowBinding {
    pub(crate) fn new(
        datagram: UdpDatagramSource,
        carrier_desc: &PacketPathCarrierDescriptor,
    ) -> Self {
        let flow_snapshot = PacketPathFlowSnapshot::from_parts(datagram.descriptor(), carrier_desc);
        Self {
            datagram,
            flow_snapshot,
        }
    }

    pub(crate) fn into_parts(self) -> (UdpDatagramSource, PacketPathFlowSnapshot) {
        (self.datagram, self.flow_snapshot)
    }
}
