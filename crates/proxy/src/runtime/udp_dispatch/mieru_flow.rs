use zero_core::Session;

use crate::runtime::udp_dispatch::{FlowFailure, UdpDispatch};
use crate::runtime::Proxy;

pub(crate) struct MieruDatagramSend<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) username: &'a str,
    pub(crate) password: &'a str,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct MieruRelaySend<'a> {
    pub(crate) session: &'a Session,
    pub(crate) carrier: crate::transport::RelayCarrier,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) username: &'a str,
    pub(crate) password: &'a str,
    pub(crate) payload: &'a [u8],
}

impl UdpDispatch {
    pub(crate) async fn send_mieru_datagram(
        &mut self,
        request: MieruDatagramSend<'_>,
    ) -> Result<usize, FlowFailure> {
        self.protocol_state
            .start_mieru_udp_flow(crate::protocol_runtime::udp::MieruUdpFlowRequest {
                chain_tasks: &mut self.chain_tasks,
                proxy: request.proxy,
                session: request.session,
                server: request.server,
                port: request.port,
                username: request.username,
                password: request.password,
                relay_chain: false,
                payload: request.payload,
            })
            .await
    }

    pub(crate) async fn send_mieru_relay(
        &mut self,
        request: MieruRelaySend<'_>,
    ) -> Result<usize, FlowFailure> {
        self.protocol_state
            .start_mieru_udp_relay_flow(
                &mut self.chain_tasks,
                crate::protocol_runtime::udp::MieruUdpRelayFlow {
                    session: request.session,
                    carrier: request.carrier,
                    server: request.server,
                    port: request.port,
                    username: request.username,
                    password: request.password,
                    payload: request.payload,
                },
            )
            .await
    }
}
