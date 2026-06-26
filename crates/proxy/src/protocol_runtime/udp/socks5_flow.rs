use crate::protocol_runtime::udp::{
    ManagedUdpFlowKind, ManagedUdpFlowRequest, ProtocolUdpFlowResume,
};
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::outbound::ManagedUdpFlowRef;
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;
use crate::runtime::Proxy;
use zero_core::Session;
use zero_engine::EngineError;

pub(crate) struct Socks5RelaySend<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) tag: &'a str,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ProtocolUdpFlowResume,
    pub(crate) session: &'a Session,
    pub(crate) payload: &'a [u8],
}

impl UdpDispatch {
    pub(crate) async fn forward_socks5_relay_flow(
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
        self.send_socks5(Socks5RelaySend {
            proxy,
            tag: flow.outbound.tag(),
            server: upstream.server,
            port: upstream.port,
            resume: self
                .managed_protocol_flow_resume(managed)
                .expect("managed relay flow should have protocol resume"),
            session: &flow.session,
            payload,
        })
        .await
    }

    /// Send via SOCKS5 upstream association, establishing one if needed.
    pub(crate) async fn send_socks5(
        &mut self,
        request: Socks5RelaySend<'_>,
    ) -> Result<usize, EngineError> {
        self.start_managed_protocol_flow(ManagedUdpFlowRequest {
            chain_tasks: None,
            proxy: Some(request.proxy),
            kind: ManagedUdpFlowKind::RelayStream,
            outbound_tag: Some(request.tag),
            session: request.session,
            carrier: None,
            tls_server_name: None,
            server: request.server,
            port: request.port,
            resume: request.resume,
            payload: request.payload,
        })
        .await
        .map_err(|failure| failure.error)
    }

    /// Start and describe a SOCKS5 UDP relay flow.
    pub(crate) async fn start_socks5_relay_flow(
        &mut self,
        request: Socks5RelaySend<'_>,
    ) -> Result<FlowStartResult, FlowFailure> {
        let resume = request.resume.clone();
        let sent = self
            .send_socks5(Socks5RelaySend {
                proxy: request.proxy,
                tag: request.tag,
                server: request.server,
                port: request.port,
                resume: request.resume,
                session: request.session,
                payload: request.payload,
            })
            .await
            .map_err(|error| FlowFailure {
                stage: "udp_upstream_send",
                error,
                upstream: Some((request.server.to_string(), request.port)),
            })?;
        let managed = self.register_managed_protocol_flow(resume);
        Ok(FlowStartResult::Flow {
            outbound: Box::new(UdpFlowOutbound::Relay {
                tag: request.tag.to_string(),
                server: request.server.to_string(),
                port: request.port,
                managed,
            }),
            tx_bytes: sent as u64,
        })
    }
}
