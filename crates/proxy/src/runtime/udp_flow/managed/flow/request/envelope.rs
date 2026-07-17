use zero_core::Session;

use super::super::resume::ManagedUdpFlowResume;
use crate::protocol_registry::UdpRuntimeServices;
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::snapshot::UdpFlowSnapshot;

pub(crate) type ManagedExistingFlowForward<'a> = (&'a UdpFlowSnapshot, &'a [u8]);

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManagedUdpFlowKind {
    #[cfg(feature = "managed-datagram-runtime")]
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
