#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
use super::model::ManagedDatagramStart;
#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use super::model::ManagedUdpSend;
#[cfg(feature = "socks5")]
use super::model::UpstreamTrackedStart;
#[cfg(any(feature = "socks5", feature = "hysteria2", feature = "shadowsocks"))]
use crate::runtime::udp_dispatch::FlowStartResult;
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
use crate::runtime::udp_flow::managed::ManagedUdpFlowResume;
use crate::runtime::udp_flow::outbound::ManagedUdpFlowRef;
#[cfg(any(feature = "socks5", feature = "hysteria2", feature = "shadowsocks"))]
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
#[cfg(feature = "socks5")]
use crate::runtime::udp_flow::registered::UpstreamAssociationSend;
use crate::runtime::udp_flow::state::UdpFlowStartContext;

impl UdpDispatch {
    pub(crate) fn flow_start_context(&mut self) -> UdpFlowStartContext<'_> {
        UdpFlowStartContext::new(&self.inbound_tag, &mut self.flow_state)
    }

    #[cfg(any(
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    pub(crate) async fn start_managed_flow(
        &mut self,
        request: ManagedUdpFlowRequest<'_>,
    ) -> Result<usize, FlowFailure> {
        self.flow_state
            .start_managed_flow(&self.inbound_tag, request)
            .await
    }

    #[cfg(any(feature = "socks5", feature = "hysteria2", feature = "shadowsocks"))]
    pub(crate) fn register_managed_flow(
        &mut self,
        resume: ManagedUdpFlowResume,
    ) -> ManagedUdpFlowRef {
        self.flow_state.register_managed_flow(resume)
    }

    #[cfg(any(
        feature = "socks5",
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    pub(crate) fn managed_flow_resume(
        &self,
        flow_ref: ManagedUdpFlowRef,
    ) -> Option<ManagedUdpFlowResume> {
        self.flow_state.managed_flow_resume(flow_ref)
    }

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
            proxy: request.proxy,
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

    #[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
    async fn start_tracked_managed_udp(
        &mut self,
        request: ManagedUdpSend<'_>,
    ) -> Result<FlowStartResult, FlowFailure> {
        let resume = request.resume.clone();
        let tag = request.tag.to_string();
        let server = request.server.to_string();
        let port = request.port;
        let sent = self.send_managed_udp(request).await?;
        let managed = self.register_managed_flow(resume);
        let outbound = UdpFlowOutbound::Datagram {
            tag,
            server,
            port,
            managed,
        };
        Ok(FlowStartResult::Flow {
            outbound: Box::new(outbound),
            tx_bytes: sent as u64,
        })
    }

    #[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
    pub(crate) async fn start_tracked_managed_datagram<T>(
        &mut self,
        request: ManagedDatagramStart<'_, T>,
    ) -> Result<FlowStartResult, FlowFailure>
    where
        T: std::any::Any + Send + Sync + std::fmt::Debug,
    {
        self.start_tracked_managed_udp(ManagedUdpSend {
            proxy: request.proxy,
            tag: request.tag,
            session: request.session,
            #[cfg(any(
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            carrier: None,
            #[cfg(any(
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            tls_server_name: None,
            server: request.server,
            port: request.port,
            resume: ManagedUdpFlowResume::new(request.resume),
            payload: request.payload,
            kind: crate::runtime::udp_flow::managed::ManagedUdpFlowKind::Datagram,
        })
        .await
    }

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
                    proxy: request.proxy,
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
