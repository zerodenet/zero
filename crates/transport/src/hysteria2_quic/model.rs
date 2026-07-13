pub struct QuicConnectionOptions<'a> {
    pub server: &'a str,
    pub port: u16,
    pub alpn: Vec<Vec<u8>>,
    pub quic_profile: Hysteria2QuicProfile,
    pub datagram_receive_buffer_size: Option<usize>,
}

#[cfg(feature = "hysteria2")]
#[derive(Debug, Clone)]
pub struct Hysteria2ManagedDatagramFlowResume {
    pub(super) protocol: hysteria2::udp::Hysteria2UdpFlowResume,
}

#[cfg(feature = "hysteria2")]
#[derive(Debug, Clone)]
pub struct OwnedHysteria2InboundProfile {
    pub(super) protocol: hysteria2::inbound::Hysteria2InboundProfile,
}

#[cfg(feature = "hysteria2")]
#[derive(Debug, Clone, Copy, Default)]
pub struct OwnedHysteria2InboundTcpResponseProtocol {
    pub(super) protocol: hysteria2::inbound::Hysteria2InboundTcpAcceptor,
}

#[cfg(feature = "hysteria2")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hysteria2ManagedUdpPacketPathCarrierDescriptor {
    pub(super) protocol: hysteria2::udp::Hysteria2UdpPacketPathCarrierDescriptor,
}

#[cfg(feature = "hysteria2")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hysteria2ManagedUdpPacketPathCarrierBuild {
    pub(super) protocol: hysteria2::udp::Hysteria2UdpPacketPathCarrierBuild,
}

#[cfg(feature = "hysteria2")]
#[derive(Debug, Clone)]
pub struct Hysteria2ManagedUdpFlowPlan<'a> {
    pub(super) tag: &'a str,
    pub(super) server: &'a str,
    pub(super) port: u16,
    pub(super) resume: Hysteria2ManagedDatagramFlowResume,
}

#[cfg(feature = "hysteria2")]
#[derive(Debug, Clone)]
pub struct Hysteria2ManagedUdpPacketPathPlan {
    pub(super) carrier_descriptor: Hysteria2ManagedUdpPacketPathCarrierDescriptor,
    pub(super) carrier_build: Hysteria2ManagedUdpPacketPathCarrierBuild,
}

#[cfg(feature = "hysteria2")]
#[derive(Debug, Clone, Copy)]
pub struct Hysteria2ManagedUdpFlowConfig<'a> {
    pub(super) tag: &'a str,
    pub(super) server: &'a str,
    pub(super) port: u16,
    pub(super) password: &'a str,
    pub(super) client_fingerprint: Option<&'a str>,
}

#[cfg(feature = "hysteria2")]
#[derive(Debug, Clone, Copy)]
pub struct Hysteria2TransportLeaf<'a> {
    pub(super) tag: &'a str,
    pub(super) server: &'a str,
    pub(super) port: u16,
    pub(super) password: &'a str,
    pub(super) client_fingerprint: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hysteria2QuicProfile {
    pub(super) client_fingerprint: Option<String>,
}
