use zero_core::Session;

use crate::protocol_runtime::udp::ProtocolUdpFlowSnapshot;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
use crate::runtime::Proxy;

pub(crate) struct TrojanDatagramSend<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) tag: &'a str,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) password: &'a str,
    pub(crate) sni: Option<&'a str>,
    pub(crate) insecure: bool,
    pub(crate) client_fingerprint: Option<&'a str>,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct TrojanRelaySend<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) tag: &'a str,
    pub(crate) session: &'a Session,
    pub(crate) carrier: crate::transport::RelayCarrier,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) password: &'a str,
    pub(crate) sni: Option<&'a str>,
    pub(crate) insecure: bool,
    pub(crate) client_fingerprint: Option<&'a str>,
    pub(crate) payload: &'a [u8],
}

impl UdpDispatch {
    pub(crate) async fn send_trojan_datagram(
        &mut self,
        request: TrojanDatagramSend<'_>,
    ) -> Result<usize, FlowFailure> {
        self.protocol_state
            .start_trojan_udp_flow(crate::protocol_runtime::udp::TrojanUdpFlowRequest {
                chain_tasks: &mut self.chain_tasks,
                proxy: request.proxy,
                session: request.session,
                server: request.server,
                port: request.port,
                password: request.password,
                sni: request.sni,
                insecure: request.insecure,
                client_fingerprint: request.client_fingerprint,
                relay_chain: false,
                payload: request.payload,
            })
            .await
    }

    pub(crate) async fn start_trojan_datagram_flow(
        &mut self,
        request: TrojanDatagramSend<'_>,
    ) -> Result<FlowStartResult, FlowFailure> {
        let sent = self
            .send_trojan_datagram(TrojanDatagramSend {
                proxy: request.proxy,
                tag: request.tag,
                session: request.session,
                server: request.server,
                port: request.port,
                password: request.password,
                sni: request.sni,
                insecure: request.insecure,
                client_fingerprint: request.client_fingerprint,
                payload: request.payload,
            })
            .await?;
        Ok(FlowStartResult::Flow {
            outbound: Box::new(UdpFlowOutbound::StreamPacket {
                tag: request.tag.to_string(),
                server: request.server.to_string(),
                port: request.port,
                protocol: ProtocolUdpFlowSnapshot::trojan(
                    request.password,
                    request.sni,
                    request.insecure,
                    request.client_fingerprint,
                    false,
                ),
            }),
            tx_bytes: sent as u64,
        })
    }

    pub(crate) async fn send_trojan_relay(
        &mut self,
        request: TrojanRelaySend<'_>,
    ) -> Result<usize, FlowFailure> {
        self.protocol_state
            .start_trojan_udp_relay_flow(crate::protocol_runtime::udp::TrojanUdpRelayFlowRequest {
                chain_tasks: &mut self.chain_tasks,
                proxy: request.proxy,
                session: request.session,
                carrier: request.carrier,
                server: request.server,
                port: request.port,
                password: request.password,
                sni: request.sni,
                insecure: request.insecure,
                client_fingerprint: request.client_fingerprint,
                payload: request.payload,
            })
            .await
    }

    pub(crate) async fn start_trojan_relay_flow(
        &mut self,
        request: TrojanRelaySend<'_>,
    ) -> Result<FlowStartResult, FlowFailure> {
        let sent = self
            .send_trojan_relay(TrojanRelaySend {
                proxy: request.proxy,
                tag: request.tag,
                session: request.session,
                carrier: request.carrier,
                server: request.server,
                port: request.port,
                password: request.password,
                sni: request.sni,
                insecure: request.insecure,
                client_fingerprint: request.client_fingerprint,
                payload: request.payload,
            })
            .await?;
        Ok(FlowStartResult::Flow {
            outbound: Box::new(UdpFlowOutbound::StreamPacket {
                tag: request.tag.to_string(),
                server: request.server.to_string(),
                port: request.port,
                protocol: ProtocolUdpFlowSnapshot::trojan(
                    request.password,
                    request.sni,
                    request.insecure,
                    request.client_fingerprint,
                    true,
                ),
            }),
            tx_bytes: sent as u64,
        })
    }
}
