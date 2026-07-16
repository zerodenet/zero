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
use crate::runtime::udp_flow::result::FlowFailure;

use super::super::model::RegisteredUdpState;
use super::error::unhandled_managed_flow;
#[cfg(all(
    feature = "socks5",
    any(
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    )
))]
use super::upstream::upstream_send;

impl RegisteredUdpState {
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
        inbound_tag: &str,
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
                .start_upstream_flow(inbound_tag, upstream_send(request))
                .await;
        }

        let result = self.managed.start_flow(request).await?;
        if let Some(sent) = result {
            return Ok(sent);
        }

        Err(unhandled_managed_flow())
    }
}
