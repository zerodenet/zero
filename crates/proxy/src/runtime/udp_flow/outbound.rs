use std::net::SocketAddr;

use zero_engine::SessionOutcome;

use crate::runtime::orchestration::UdpPathCategory;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ManagedUdpFlowRef(u64);

impl ManagedUdpFlowRef {
    pub(crate) fn new(id: u64) -> Self {
        Self(id)
    }
}

/// Outbound type tracked per UDP flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UdpFlowOutbound {
    Direct {
        tag: String,
        target_addr: SocketAddr,
    },
    Relay {
        tag: String,
        server: String,
        port: u16,
        managed: ManagedUdpFlowRef,
    },
    Datagram {
        tag: String,
        server: String,
        port: u16,
        managed: ManagedUdpFlowRef,
    },
    StreamPacket {
        tag: String,
        server: String,
        port: u16,
        managed: ManagedUdpFlowRef,
    },
    PacketPathDatagram {
        tag: String,
        server: String,
        port: u16,
        snapshot: crate::runtime::udp_flow::packet_path::PacketPathFlowSnapshot,
    },
}

pub(super) struct UdpFlowIndexKeys<'a> {
    pub(super) direct_sender: Option<SocketAddr>,
    pub(super) upstream_response_tag: Option<&'a str>,
}

pub(super) struct UdpFlowCompletion {
    pub(super) upstream: Option<(String, u16)>,
    pub(super) success_outcome: SessionOutcome,
}

pub(crate) struct UdpFlowUpstream<'a> {
    pub(crate) server: &'a str,
    pub(crate) port: u16,
}

impl UdpFlowOutbound {
    pub(crate) fn tag(&self) -> &str {
        match self {
            Self::Direct { tag, .. }
            | Self::Relay { tag, .. }
            | Self::Datagram { tag, .. }
            | Self::StreamPacket { tag, .. }
            | Self::PacketPathDatagram { tag, .. } => tag,
        }
    }

    /// Return the path category for this outbound.
    pub(crate) fn path_category(&self) -> UdpPathCategory {
        match self {
            Self::Direct { .. } => UdpPathCategory::Direct,
            Self::Relay { .. } => UdpPathCategory::Relay,
            Self::Datagram { .. } => UdpPathCategory::Datagram,
            Self::StreamPacket { .. } => UdpPathCategory::StreamPacket,
            Self::PacketPathDatagram { .. } => UdpPathCategory::PacketPathDatagram,
        }
    }

    pub(crate) fn direct_target_addr(&self) -> Option<SocketAddr> {
        match self {
            Self::Direct { target_addr, .. } => Some(*target_addr),
            Self::Relay { .. }
            | Self::Datagram { .. }
            | Self::StreamPacket { .. }
            | Self::PacketPathDatagram { .. } => None,
        }
    }

    pub(crate) fn relay_managed_flow(&self) -> Option<ManagedUdpFlowRef> {
        match self {
            Self::Relay { managed, .. } => Some(*managed),
            Self::Direct { .. }
            | Self::Datagram { .. }
            | Self::StreamPacket { .. }
            | Self::PacketPathDatagram { .. } => None,
        }
    }

    pub(crate) fn managed_flow(&self) -> Option<ManagedUdpFlowRef> {
        match self {
            Self::Datagram { managed, .. } | Self::StreamPacket { managed, .. } => Some(*managed),
            Self::Direct { .. } | Self::Relay { .. } | Self::PacketPathDatagram { .. } => None,
        }
    }

    pub(crate) fn packet_path_snapshot(
        &self,
    ) -> Option<&crate::runtime::udp_flow::packet_path::PacketPathFlowSnapshot> {
        match self {
            Self::PacketPathDatagram { snapshot, .. } => Some(snapshot),
            Self::Direct { .. }
            | Self::Relay { .. }
            | Self::Datagram { .. }
            | Self::StreamPacket { .. } => None,
        }
    }

    pub(crate) fn upstream(&self) -> Option<UdpFlowUpstream<'_>> {
        match self {
            Self::Direct { .. } => None,
            Self::Relay { server, port, .. }
            | Self::Datagram { server, port, .. }
            | Self::StreamPacket { server, port, .. }
            | Self::PacketPathDatagram { server, port, .. } => Some(UdpFlowUpstream {
                server,
                port: *port,
            }),
        }
    }

    pub(super) fn index_keys(&self) -> UdpFlowIndexKeys<'_> {
        UdpFlowIndexKeys {
            direct_sender: self.direct_target_addr(),
            upstream_response_tag: self.upstream_response_tag(),
        }
    }

    fn upstream_response_tag(&self) -> Option<&str> {
        match self {
            Self::Direct { .. } => None,
            Self::Relay { tag, .. }
            | Self::Datagram { tag, .. }
            | Self::StreamPacket { tag, .. }
            | Self::PacketPathDatagram { tag, .. } => Some(tag),
        }
    }

    fn upstream_endpoint(&self) -> Option<(String, u16)> {
        match self {
            Self::Direct { .. } => None,
            Self::Relay { server, port, .. }
            | Self::Datagram { server, port, .. }
            | Self::StreamPacket { server, port, .. }
            | Self::PacketPathDatagram { server, port, .. } => Some((server.clone(), *port)),
        }
    }

    fn success_outcome(&self) -> SessionOutcome {
        match self {
            Self::Direct { .. } => SessionOutcome::DirectRelayed,
            Self::Relay { .. }
            | Self::Datagram { .. }
            | Self::StreamPacket { .. }
            | Self::PacketPathDatagram { .. } => SessionOutcome::ChainedRelayed,
        }
    }

    pub(super) fn completion(&self) -> UdpFlowCompletion {
        UdpFlowCompletion {
            upstream: self.upstream_endpoint(),
            success_outcome: self.success_outcome(),
        }
    }
}
