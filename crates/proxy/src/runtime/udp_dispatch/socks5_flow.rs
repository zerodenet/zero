use crate::protocol_runtime::udp::{
    ManagedUdpFlowKind, ManagedUdpFlowRequest, ProtocolUdpFlowResume, ProtocolUdpFlowSnapshot,
};
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
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
    /// Send via SOCKS5 upstream association, establishing one if needed.
    pub(crate) async fn send_socks5(
        &mut self,
        request: Socks5RelaySend<'_>,
    ) -> Result<usize, EngineError> {
        self.protocol_state
            .start_managed_udp_flow(
                &self.inbound_tag,
                ManagedUdpFlowRequest {
                    chain_tasks: &mut self.chain_tasks,
                    proxy: Some(request.proxy),
                    kind: ManagedUdpFlowKind::RelayStream,
                    session: request.session,
                    carrier: None,
                    tls_server_name: None,
                    server: request.server,
                    port: request.port,
                    resume: request.resume,
                    payload: request.payload,
                },
            )
            .await
            .map_err(|failure| failure.error)
    }

    /// Start and describe a SOCKS5 UDP relay flow.
    pub(crate) async fn start_socks5_relay_flow(
        &mut self,
        request: Socks5RelaySend<'_>,
    ) -> Result<FlowStartResult, FlowFailure> {
        let protocol = ProtocolUdpFlowSnapshot::managed(request.resume.clone());
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
        Ok(FlowStartResult::Flow {
            outbound: Box::new(UdpFlowOutbound::Relay {
                tag: request.tag.to_string(),
                server: request.server.to_string(),
                port: request.port,
                protocol,
            }),
            tx_bytes: sent as u64,
        })
    }
}
