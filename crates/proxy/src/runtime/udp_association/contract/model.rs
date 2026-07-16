use std::net::SocketAddr;

use zero_platform_tokio::TokioDatagramSocket;

use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_ingress::UdpIngressRuntime;
use crate::transport::{MeteredStream, StreamTraffic};

pub(crate) struct UdpAssociationDatagramRequest<'a> {
    pub(crate) runtime: &'a UdpIngressRuntime,
    pub(crate) dispatch: &'a mut UdpDispatch,
    pub(crate) relay: &'a TokioDatagramSocket,
    pub(crate) pending_control_traffic: &'a mut StreamTraffic,
    pub(crate) sender: SocketAddr,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct UdpAssociationLoopRequest<'a, S, H> {
    pub(crate) runtime: UdpIngressRuntime,
    pub(crate) client: &'a mut MeteredStream<S>,
    pub(crate) inbound_tag: &'a str,
    pub(crate) relay: TokioDatagramSocket,
    pub(crate) pending_control_traffic: StreamTraffic,
    pub(crate) handler: H,
}
