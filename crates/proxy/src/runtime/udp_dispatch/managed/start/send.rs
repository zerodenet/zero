#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use crate::runtime::udp_dispatch::managed::model::ManagedUdpSend;
use crate::runtime::udp_dispatch::{FlowFailure, UdpDispatch};
#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use crate::runtime::udp_flow::managed::ManagedUdpFlowRequest;

impl UdpDispatch {
    #[cfg(any(
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    pub(in crate::runtime::udp_dispatch::managed) async fn send_managed_udp(
        &mut self,
        request: ManagedUdpSend<'_>,
    ) -> Result<usize, FlowFailure> {
        self.start_managed_flow(ManagedUdpFlowRequest {
            chain_tasks: None,
            services: request.services,
            kind: request.kind,
            session: request.session,
            #[cfg(any(
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            carrier: request.carrier,
            #[cfg(any(
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            tls_server_name: request.tls_server_name,
            server: request.server,
            port: request.port,
            resume: request.resume,
            payload: request.payload,
        })
        .await
    }
}
