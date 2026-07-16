#[cfg(feature = "socks5")]
use crate::runtime::udp_dispatch::managed::model::UpstreamTrackedStart;
#[cfg(any(feature = "socks5", feature = "hysteria2", feature = "shadowsocks"))]
use crate::runtime::udp_dispatch::FlowStartResult;
use crate::runtime::udp_dispatch::{FlowFailure, UdpDispatch};
#[cfg(feature = "socks5")]
use crate::runtime::udp_flow::managed::ManagedUdpFlowResume;
#[cfg(any(feature = "socks5", feature = "hysteria2", feature = "shadowsocks"))]
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
#[cfg(feature = "socks5")]
use crate::runtime::udp_flow::registered::UpstreamAssociationSend;

impl UdpDispatch {
    #[cfg(feature = "socks5")]
    pub(crate) async fn start_tracked_upstream<T>(
        &mut self,
        request: UpstreamTrackedStart<'_, T>,
    ) -> Result<FlowStartResult, FlowFailure>
    where
        T: std::any::Any + Send + Sync + std::fmt::Debug,
    {
        let resume = ManagedUdpFlowResume::new(request.resume);
        let sent = self
            .flow_state
            .start_upstream_flow(
                &self.inbound_tag,
                UpstreamAssociationSend {
                    services: request.services,
                    session: request.session,
                    server: request.server,
                    port: request.port,
                    resume: resume.clone(),
                    payload: request.payload,
                },
            )
            .await?;
        let managed = self.register_managed_flow(resume);
        Ok(FlowStartResult::Flow {
            outbound: Box::new(UdpFlowOutbound::Relay {
                tag: request.tag.to_owned(),
                server: request.server.to_owned(),
                port: request.port,
                managed,
            }),
            tx_bytes: sent as u64,
        })
    }
}
