use zero_core::Session;

use crate::runtime::udp_dispatch::{FlowFailure, UdpDispatch};
use crate::runtime::Proxy;

pub(crate) struct TrojanDatagramSend<'a> {
    pub(crate) proxy: &'a Proxy,
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
}
