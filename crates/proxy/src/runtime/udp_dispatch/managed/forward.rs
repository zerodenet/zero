use crate::runtime::udp_dispatch::UdpDispatch;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
use crate::runtime::udp_flow::outbound::ManagedUdpFlowRef;
#[cfg(feature = "socks5")]
use crate::runtime::udp_flow::registered::UpstreamAssociationSend;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
use crate::runtime::udp_flow::snapshot::UdpFlowSnapshot;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
use crate::runtime::Proxy;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
use zero_engine::EngineError;

impl UdpDispatch {
    #[cfg(any(
        feature = "socks5",
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
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
        #[cfg(feature = "socks5")]
        let resume = self
            .managed_flow_resume(managed)
            .expect("managed relay flow should have protocol resume");
        #[cfg(feature = "socks5")]
        if self.flow_state.handles_upstream_resume(&resume) {
            return self
                .flow_state
                .start_upstream_flow(
                    &self.inbound_tag,
                    UpstreamAssociationSend {
                        proxy: Some(proxy),
                        session: &flow.session,
                        server: upstream.server,
                        port: upstream.port,
                        resume,
                        payload,
                    },
                )
                .await
                .map_err(|failure| failure.error);
        }
        #[cfg(not(feature = "socks5"))]
        let _ = managed;
        #[cfg(any(
            feature = "vless",
            feature = "vmess",
            feature = "trojan",
            feature = "mieru"
        ))]
        return self
            .flow_state
            .forward_existing_managed_flow(proxy, (flow, payload))
            .await
            .map_err(|failure| failure.error);

        #[cfg(not(any(
            feature = "vless",
            feature = "vmess",
            feature = "trojan",
            feature = "mieru"
        )))]
        Err(EngineError::Io(std::io::Error::other(
            "registered upstream flow resume is not handled by the compiled adapter",
        )))
    }
}
