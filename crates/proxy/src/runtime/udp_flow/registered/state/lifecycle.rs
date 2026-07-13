#[cfg(feature = "socks5")]
use std::time::Duration;

#[cfg(feature = "socks5")]
use tokio::time::Instant as TokioInstant;
#[cfg(feature = "socks5")]
use zero_engine::EngineError;

#[cfg(feature = "socks5")]
use super::model::ClosedRegisteredUpstreamAssociation;
#[cfg(feature = "socks5")]
use super::model::RegisteredUpstreamAssociationView;
use super::model::{RegisteredUdpHandlers, RegisteredUdpState};
use crate::runtime::udp_flow::managed::ManagedUdpFlowResume;
use crate::runtime::udp_flow::outbound::ManagedUdpFlowRef;
#[cfg(feature = "socks5")]
use crate::runtime::udp_flow::response::UpstreamUdpResponse;

impl RegisteredUdpState {
    pub(crate) fn new(handlers: RegisteredUdpHandlers) -> Self {
        Self {
            #[cfg(any(
                feature = "vless",
                feature = "hysteria2",
                feature = "shadowsocks",
                feature = "trojan",
                feature = "vmess",
                feature = "mieru"
            ))]
            managed: super::super::super::managed::ManagedUdpState::new(handlers.managed),
            #[cfg(feature = "socks5")]
            upstream: super::super::upstream::UpstreamAssociationState::new(handlers.upstream),
            managed_resumes: std::collections::HashMap::new(),
            next_managed_flow_id: 1,
        }
    }

    pub(crate) fn register_managed_flow(
        &mut self,
        resume: ManagedUdpFlowResume,
    ) -> ManagedUdpFlowRef {
        let flow_ref = ManagedUdpFlowRef::new(self.next_managed_flow_id);
        self.next_managed_flow_id += 1;
        self.managed_resumes.insert(flow_ref, resume);
        flow_ref
    }

    pub(crate) fn managed_flow_resume(
        &self,
        flow_ref: ManagedUdpFlowRef,
    ) -> Option<ManagedUdpFlowResume> {
        self.managed_resumes.get(&flow_ref).cloned()
    }

    #[cfg(feature = "socks5")]
    pub(crate) async fn recv_upstream_response(
        &self,
        buf: &mut [u8],
    ) -> Result<UpstreamUdpResponse, EngineError> {
        self.upstream.recv_upstream_response(buf).await
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn upstream_association_view(
        &self,
    ) -> Option<RegisteredUpstreamAssociationView<'_>> {
        self.upstream
            .upstream_outbound_tag()
            .map(|outbound_tag| RegisteredUpstreamAssociationView { outbound_tag })
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn upstream_idle_deadline(&self) -> Option<TokioInstant> {
        self.upstream.upstream_idle_deadline()
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn touch_upstream_idle(&mut self, timeout: Duration) {
        self.upstream.touch_upstream_idle(timeout);
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn drop_upstream_association(
        &mut self,
    ) -> Option<ClosedRegisteredUpstreamAssociation> {
        self.upstream
            .drop_upstream_association()
            .map(closed_registered_upstream_association)
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn close_idle_upstream(&mut self) -> Option<ClosedRegisteredUpstreamAssociation> {
        self.upstream
            .close_idle_upstream()
            .map(closed_registered_upstream_association)
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn close_all_upstreams(mut self) {
        self.upstream.close_all_upstreams();
    }
}

#[cfg(feature = "socks5")]
fn closed_registered_upstream_association(
    (outbound_tag, server, port): (String, String, u16),
) -> ClosedRegisteredUpstreamAssociation {
    ClosedRegisteredUpstreamAssociation {
        outbound_tag,
        server,
        port,
    }
}
