//! Shadowsocks UDP relay: protocol framing and routing through the UDP pipe.

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
}

#[async_trait::async_trait]
impl DatagramUdpResponder<Arc<UdpSocket>> for ShadowsocksDatagramUdpResponder {
    async fn read_inbound_dispatch(
        &mut self,
        udp_socket: &Arc<UdpSocket>,
    ) -> Result<Option<InboundUdpDispatch>, zero_core::Error> {
        self.inner
            .read_inbound_dispatch_from_socket_tokio(udp_socket.as_ref())
            .await
            .map(Some)
    }

    fn auth(&self) -> Option<&SessionAuth> {
        Some(&self.auth)
    }

    fn on_dispatch_success(&mut self, session_id: u64, dispatch: &InboundUdpDispatch) {
        self.inner
            .record_pending_dispatch_success(session_id, dispatch.client_session_id());
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
                },
                inbound_tag,
                poll_upstream: false,
            },
        )
        .await
    }
}
