use zero_core::Session;

use crate::runtime::udp_dispatch::{FlowFailure, UdpDispatch};
use crate::runtime::Proxy;

pub(crate) struct VmessDatagramSend<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) uuid: [u8; 16],
    pub(crate) cipher_name: &'a str,
    pub(crate) cipher: vmess::VmessCipher,
    pub(crate) mux_concurrency: Option<u32>,
    pub(crate) tls: Option<&'a zero_config::ClientTlsConfig>,
    pub(crate) ws: Option<&'a zero_config::WebSocketConfig>,
    pub(crate) grpc: Option<&'a zero_config::GrpcConfig>,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct VmessRelaySend<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) carrier: crate::transport::RelayCarrier,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) uuid: [u8; 16],
    pub(crate) cipher: vmess::VmessCipher,
    pub(crate) tls: Option<&'a zero_config::ClientTlsConfig>,
    pub(crate) ws: Option<&'a zero_config::WebSocketConfig>,
    pub(crate) grpc: Option<&'a zero_config::GrpcConfig>,
    pub(crate) payload: &'a [u8],
}

impl UdpDispatch {
    pub(crate) async fn send_vmess_datagram(
        &mut self,
        request: VmessDatagramSend<'_>,
    ) -> Result<(), FlowFailure> {
        self.protocol_state
            .start_vmess_udp_flow(
                &mut self.chain_tasks,
                crate::protocol_runtime::udp::VmessUdpFlow {
                    proxy: request.proxy,
                    session: request.session,
                    server: request.server,
                    port: request.port,
                    uuid: request.uuid,
                    cipher_name: request.cipher_name,
                    cipher: request.cipher,
                    mux_concurrency: request.mux_concurrency,
                    tls: request.tls,
                    ws: request.ws,
                    grpc: request.grpc,
                    payload: request.payload,
                },
            )
            .await
    }

    pub(crate) async fn send_vmess_relay(
        &mut self,
        request: VmessRelaySend<'_>,
    ) -> Result<(), FlowFailure> {
        self.protocol_state
            .start_vmess_udp_relay_flow(
                &mut self.chain_tasks,
                crate::protocol_runtime::udp::VmessUdpRelayFlow {
                    proxy: request.proxy,
                    session: request.session,
                    carrier: request.carrier,
                    server: request.server,
                    port: request.port,
                    uuid: request.uuid,
                    cipher: request.cipher,
                    tls: request.tls,
                    ws: request.ws,
                    grpc: request.grpc,
                    payload: request.payload,
                },
            )
            .await
    }
}
