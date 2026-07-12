use super::model::{ManagedUdpOutboundKind, ManagedUdpSend};
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_flow::managed::ManagedUdpFlowKind;
use crate::runtime::udp_flow::outbound::ManagedUdpFlowRef;
use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;
use crate::runtime::Proxy;
use zero_engine::EngineError;

impl UdpDispatch {
    pub(in crate::runtime::udp_dispatch) async fn forward_managed_relay_flow(
        &mut self,
        proxy: &Proxy,
        flow: &UdpFlowSnapshot,
        managed: ManagedUdpFlowRef,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        let upstream = flow
            .outbound
            .upstream()
            .expect("relay flow should expose upstream endpoint");
        let resume = self
            .managed_flow_resume(managed)
            .expect("managed relay flow should have protocol resume");
        self.send_managed_udp(ManagedUdpSend {
            proxy: Some(proxy),
            tag: flow.outbound.tag(),
            session: &flow.session,
            carrier: None,
            tls_server_name: None,
            server: upstream.server,
            port: upstream.port,
            resume,
            payload,
            kind: ManagedUdpFlowKind::RelayStream,
            outbound: ManagedUdpOutboundKind::Relay,
        })
        .await
        .map_err(|failure| failure.error)
    }
}
