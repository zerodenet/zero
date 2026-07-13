#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use zero_engine::EngineError;

use super::model::RegisteredUdpState;
#[cfg(all(
    feature = "socks5",
    any(
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    )
))]
use crate::runtime::udp_flow::managed::ManagedUdpFlowKind;
#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use crate::runtime::udp_flow::managed::ManagedUdpFlowRequest;
#[cfg(feature = "socks5")]
use crate::runtime::udp_flow::registered::upstream::UpstreamAssociationSend;
use crate::runtime::udp_flow::result::FlowFailure;

impl RegisteredUdpState {
    #[cfg(feature = "socks5")]
    pub(crate) async fn start_upstream_udp_flow(
        &mut self,
        inbound_tag: &str,
        request: UpstreamAssociationSend<'_>,
    ) -> Result<usize, FlowFailure> {
        self.upstream
            .start_upstream_flow(inbound_tag, request)
            .await
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn handles_upstream_resume(
        &self,
        resume: &crate::runtime::udp_flow::managed::ManagedUdpFlowResume,
    ) -> bool {
        self.upstream.handles_resume(resume)
    }

    #[cfg(any(
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    pub(crate) async fn start_managed_udp_flow(
        &mut self,
        _inbound_tag: &str,
        request: ManagedUdpFlowRequest<'_>,
    ) -> Result<usize, FlowFailure> {
        #[cfg(all(
            feature = "socks5",
            any(
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            )
        ))]
        if matches!(request.kind, ManagedUdpFlowKind::RelayStream) && request.carrier.is_none() {
            return self
                .upstream
                .start_upstream_flow(_inbound_tag, upstream_send(request))
                .await;
        }
        let result = self.managed.start_flow(request).await?;
        if let Some(sent) = result {
            return Ok(sent);
        }
        Err(unhandled_managed_flow())
    }
}

#[cfg(all(
    feature = "socks5",
    any(
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    )
))]
fn upstream_send(request: ManagedUdpFlowRequest<'_>) -> UpstreamAssociationSend<'_> {
    UpstreamAssociationSend {
        proxy: request.proxy,
        session: request.session,
        server: request.server,
        port: request.port,
        resume: request.resume,
        payload: request.payload,
    }
}

#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
fn unhandled_managed_flow() -> FlowFailure {
    FlowFailure {
        stage: "udp_managed_flow_start",
        error: EngineError::Io(std::io::Error::other(
            "managed UDP flow request had no compiled start handler",
        )),
        upstream: None,
    }
}
