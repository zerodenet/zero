use tokio::task::JoinSet;
use zero_core::Address;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
use zero_core::Session;

use crate::protocol_registry::UdpRuntimeServices;
#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
use crate::runtime::udp_flow::managed::flow::ManagedDatagramFlow;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
use crate::runtime::udp_flow::managed::flow::ManagedRelayStreamFlow;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
use crate::runtime::udp_flow::managed::flow::ManagedStreamPacketFlow;
use crate::runtime::udp_flow::managed::flow::ManagedUdpFlowResume;
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::snapshot::UdpFlowSnapshot;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
use crate::transport::TcpRelayStream;

#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
pub(crate) struct ManagedDatagramExistingSend<'a> {
    pub(crate) chain_tasks: &'a mut JoinSet<ChainTask>,
    pub(crate) session_id: u64,
    pub(crate) services: Option<UdpRuntimeServices>,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ManagedUdpFlowResume,
    pub(crate) target: &'a Address,
    pub(crate) target_port: u16,
    pub(crate) payload: &'a [u8],
}

#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
impl<'a> ManagedDatagramExistingSend<'a> {
    pub(crate) fn datagram(
        chain_tasks: &'a mut JoinSet<ChainTask>,
        flow: &ManagedDatagramFlow<'a>,
    ) -> Self {
        Self {
            chain_tasks,
            session_id: flow.session.id,
            services: flow.services.clone(),
            server: flow.server,
            port: flow.port,
            resume: flow.resume.clone(),
            target: &flow.session.target,
            target_port: flow.session.port,
            payload: flow.payload,
        }
    }

    pub(crate) fn forwarded(
        chain_tasks: &'a mut JoinSet<ChainTask>,
        services: UdpRuntimeServices,
        flow: &'a UdpFlowSnapshot,
        resume: ManagedUdpFlowResume,
        server: &'a str,
        port: u16,
        payload: &'a [u8],
    ) -> Self {
        Self {
            chain_tasks,
            session_id: flow.session.id,
            services: Some(services),
            server,
            port,
            resume,
            target: &flow.session.target,
            target_port: flow.session.port,
            payload,
        }
    }
}

#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
pub(crate) struct ManagedStreamExistingSend<'a> {
    pub(crate) chain_tasks: &'a mut JoinSet<ChainTask>,
    pub(crate) session_id: u64,
    pub(crate) services: UdpRuntimeServices,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ManagedUdpFlowResume,
    pub(crate) target: &'a Address,
    pub(crate) target_port: u16,
    pub(crate) payload: &'a [u8],
}

#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
impl<'a> ManagedStreamExistingSend<'a> {
    pub(crate) fn stream_packet(request: ManagedStreamPacketFlow<'a>) -> Self {
        Self {
            chain_tasks: request.chain_tasks,
            session_id: request.session.id,
            services: request.services,
            session: request.session,
            server: request.server,
            port: request.port,
            resume: request.resume,
            target: &request.session.target,
            target_port: request.session.port,
            payload: request.payload,
        }
    }

    pub(crate) fn forwarded(
        chain_tasks: &'a mut JoinSet<ChainTask>,
        services: UdpRuntimeServices,
        flow: &'a UdpFlowSnapshot,
        resume: ManagedUdpFlowResume,
        server: &'a str,
        port: u16,
        payload: &'a [u8],
    ) -> Self {
        Self {
            chain_tasks,
            session_id: flow.session.id,
            services,
            session: &flow.session,
            server,
            port,
            resume,
            target: &flow.session.target,
            target_port: flow.session.port,
            payload,
        }
    }
}

#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
pub(crate) struct ManagedRelayExistingSend<'a> {
    pub(crate) chain_tasks: &'a mut JoinSet<ChainTask>,
    pub(crate) session_id: u64,
    pub(crate) stream: TcpRelayStream,
    pub(crate) tls_server_name: Option<&'a str>,
    pub(crate) services: Option<UdpRuntimeServices>,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ManagedUdpFlowResume,
    pub(crate) target: &'a Address,
    pub(crate) target_port: u16,
    pub(crate) payload: &'a [u8],
}

#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
impl<'a> ManagedRelayExistingSend<'a> {
    pub(crate) fn relay_stream(request: ManagedRelayStreamFlow<'a>) -> Self {
        Self {
            chain_tasks: request.chain_tasks,
            session_id: request.session.id,
            stream: request.carrier.stream,
            tls_server_name: request.tls_server_name,
            services: request.services,
            session: request.session,
            server: request.server,
            port: request.port,
            resume: request.resume,
            target: &request.session.target,
            target_port: request.session.port,
            payload: request.payload,
        }
    }
}
