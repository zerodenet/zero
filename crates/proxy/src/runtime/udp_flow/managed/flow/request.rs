#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use zero_core::Session;

#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use super::resume::ManagedUdpFlowResume;
#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
use crate::protocol_registry::UdpRuntimeServices;
#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use crate::runtime::udp_flow::packet_path::ChainTask;
#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use crate::runtime::udp_flow::snapshot::UdpFlowSnapshot;

#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) type ManagedExistingFlowForward<'a> = (&'a UdpFlowSnapshot, &'a [u8]);

#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
pub(crate) struct ManagedDatagramFlow<'a> {
    pub(crate) services: Option<UdpRuntimeServices>,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ManagedUdpFlowResume,
    pub(crate) payload: &'a [u8],
}

#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
pub(crate) struct ManagedStreamPacketFlow<'a> {
    pub(crate) chain_tasks: &'a mut tokio::task::JoinSet<ChainTask>,
    pub(crate) services: UdpRuntimeServices,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ManagedUdpFlowResume,
    pub(crate) payload: &'a [u8],
}

#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
pub(crate) struct ManagedRelayStreamFlow<'a> {
    pub(crate) chain_tasks: &'a mut tokio::task::JoinSet<ChainTask>,
    pub(crate) services: Option<UdpRuntimeServices>,
    pub(crate) session: &'a Session,
    pub(crate) carrier: crate::transport::RelayCarrier,
    pub(crate) tls_server_name: Option<&'a str>,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ManagedUdpFlowResume,
    pub(crate) payload: &'a [u8],
}

#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) struct ManagedUdpFlowRequest<'a> {
    pub(crate) chain_tasks: Option<&'a mut tokio::task::JoinSet<ChainTask>>,
    pub(crate) services: Option<UdpRuntimeServices>,
    pub(crate) kind: ManagedUdpFlowKind,
    pub(crate) session: &'a Session,
    #[cfg(any(
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    pub(crate) carrier: Option<crate::transport::RelayCarrier>,
    #[cfg(any(
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    pub(crate) tls_server_name: Option<&'a str>,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ManagedUdpFlowResume,
    pub(crate) payload: &'a [u8],
}

#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManagedUdpFlowKind {
    #[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
    Datagram,
    #[cfg(any(
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    StreamPacket,
    #[cfg(any(
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    RelayStream,
}
