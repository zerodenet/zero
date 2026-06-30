//! Shadowsocks UDP relay: protocol framing and routing through the UDP pipe.

use std::net::SocketAddr;
use std::sync::Arc;

use shadowsocks::ShadowsocksInboundProfile;
use tokio::net::UdpSocket;
use zero_core::{InboundUdpDispatch, SessionAuth};
use zero_engine::EngineError;

use crate::inbound::datagram_udp::{
    run_datagram_udp_relay, DatagramUdpRelayRequest, DatagramUdpResponder,
};
use crate::runtime::Proxy;

struct ShadowsocksDatagramUdpResponder {
    inner: shadowsocks::ShadowsocksInboundUdpResponder,
    auth: SessionAuth,
    pending_client_addr: Option<SocketAddr>,
}

#[async_trait::async_trait]
impl DatagramUdpResponder<Arc<UdpSocket>> for ShadowsocksDatagramUdpResponder {
    async fn read_inbound_dispatch(
        &mut self,
        udp_socket: &Arc<UdpSocket>,
    ) -> Result<Option<InboundUdpDispatch>, zero_core::Error> {
        let mut buf = [0u8; 65536];
        loop {
            let (n, client_addr) = udp_socket
                .recv_from(&mut buf)
                .await
                .map_err(|_| zero_core::Error::Io("ss udp recv error"))?;
            match self.inner.decode_inbound_dispatch(&buf[..n]) {
                Ok(inbound_dispatch) => {
                    self.pending_client_addr = Some(client_addr);
                    return Ok(Some(inbound_dispatch));
                }
                Err(_) => continue,
            }
        }
    }

    fn auth(&self) -> Option<&SessionAuth> {
        Some(&self.auth)
    }

    fn on_dispatch_success(&mut self, session_id: u64, dispatch: &InboundUdpDispatch) {
        if let Some(client_addr) = self.pending_client_addr.take() {
            self.inner.record_dispatch_success(
                session_id,
                dispatch.client_session_id(),
                client_addr,
            );
        }
    }

    async fn write_response_for_session(
        &mut self,
        udp_socket: &Arc<UdpSocket>,
        session_id: Option<u64>,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<usize>, zero_core::Error> {
        self.inner
            .send_response_for_target_proxy_session_to_client_tokio(
                udp_socket.as_ref(),
                session_id,
                target,
                port,
                payload,
            )
            .await
    }
}

impl Proxy {
    pub(crate) async fn ss_udp_relay_loop(
        &self,
        udp_socket: Arc<UdpSocket>,
        inbound_tag: &str,
        profile: ShadowsocksInboundProfile,
    ) -> Result<(), EngineError> {
        run_datagram_udp_relay(
            self,
            DatagramUdpRelayRequest {
                source: udp_socket,
                responder: ShadowsocksDatagramUdpResponder {
                    inner: profile.udp_responder(),
                    auth: profile.inbound_auth(),
                    pending_client_addr: None,
                },
                inbound_tag,
                poll_upstream: false,
            },
        )
        .await
    }
}
