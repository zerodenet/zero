use zero_core::Session;
use zero_engine::EngineError;

use crate::runtime::tcp_ingress::TcpIngressRuntime;
use crate::runtime::udp_ingress::UdpIngressRuntime;

#[derive(Clone)]
pub(crate) struct MuxSubstreamRuntime {
    tcp_runtime: TcpIngressRuntime,
    udp_runtime: UdpIngressRuntime,
}

impl MuxSubstreamRuntime {
    pub(crate) fn new(tcp_runtime: TcpIngressRuntime, udp_runtime: UdpIngressRuntime) -> Self {
        Self {
            tcp_runtime,
            udp_runtime,
        }
    }

    pub(crate) fn inbound_tag(&self) -> &str {
        self.tcp_runtime.inbound_tag()
    }

    pub(crate) fn udp_runtime(&self) -> UdpIngressRuntime {
        self.udp_runtime.clone()
    }

    pub(crate) async fn open_tcp_upstream(
        &self,
        session: &mut Session,
    ) -> Result<crate::transport::TcpRouteResult, EngineError> {
        self.tcp_runtime.open_tcp_upstream(session).await
    }
}
