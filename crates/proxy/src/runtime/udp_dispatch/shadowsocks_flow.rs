use zero_core::Session;

use crate::runtime::udp_dispatch::{FlowFailure, UdpDispatch};
use crate::runtime::Proxy;

pub(crate) struct ShadowsocksDatagramSend<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) password: &'a str,
    pub(crate) cipher: shadowsocks::CipherKind,
    pub(crate) payload: &'a [u8],
}

impl UdpDispatch {
    pub(crate) async fn send_shadowsocks_datagram(
        &mut self,
        request: ShadowsocksDatagramSend<'_>,
    ) -> Result<usize, FlowFailure> {
        self.protocol_state
            .start_shadowsocks_udp_flow(
                &mut self.chain_tasks,
                crate::protocol_runtime::udp::ShadowsocksUdpFlow {
                    proxy: request.proxy,
                    session: request.session,
                    server: request.server,
                    port: request.port,
                    password: request.password,
                    cipher: request.cipher,
                    payload: request.payload,
                },
            )
            .await
    }
}
