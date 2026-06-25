use zero_core::Session;

use crate::runtime::udp_dispatch::{FlowFailure, UdpDispatch};
use crate::runtime::Proxy;

pub(crate) struct VlessDatagramSend<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) uuid: [u8; 16],
    pub(crate) flow: Option<&'a str>,
    pub(crate) tls: Option<&'a zero_config::ClientTlsConfig>,
    pub(crate) reality: Option<&'a zero_config::RealityConfig>,
    pub(crate) ws: Option<&'a zero_config::WebSocketConfig>,
    pub(crate) grpc: Option<&'a zero_config::GrpcConfig>,
    pub(crate) h2: Option<&'a zero_config::H2Config>,
    pub(crate) http_upgrade: Option<&'a zero_config::HttpUpgradeConfig>,
    pub(crate) split_http: Option<&'a zero_config::SplitHttpConfig>,
    pub(crate) quic: Option<&'a zero_config::QuicConfig>,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct VlessRelayTwoStreamSend<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) post_carrier: crate::transport::RelayCarrier,
    pub(crate) get_carrier: crate::transport::RelayCarrier,
    pub(crate) uuid: [u8; 16],
    pub(crate) split_http: &'a zero_config::SplitHttpConfig,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct VlessRelayFinalHopSend<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) carrier: crate::transport::RelayCarrier,
    pub(crate) uuid: [u8; 16],
    pub(crate) tls: Option<&'a zero_config::ClientTlsConfig>,
    pub(crate) reality: Option<&'a zero_config::RealityConfig>,
    pub(crate) ws: Option<&'a zero_config::WebSocketConfig>,
    pub(crate) grpc: Option<&'a zero_config::GrpcConfig>,
    pub(crate) h2: Option<&'a zero_config::H2Config>,
    pub(crate) http_upgrade: Option<&'a zero_config::HttpUpgradeConfig>,
    pub(crate) split_http: Option<&'a zero_config::SplitHttpConfig>,
    pub(crate) payload: &'a [u8],
}

impl UdpDispatch {
    pub(crate) async fn send_vless_datagram(
        &mut self,
        request: VlessDatagramSend<'_>,
    ) -> Result<(), FlowFailure> {
        self.protocol_state
            .start_vless_udp_flow(
                &mut self.chain_tasks,
                crate::protocol_runtime::udp::VlessUdpFlow {
                    proxy: request.proxy,
                    session: request.session,
                    server: request.server,
                    port: request.port,
                    uuid: request.uuid,
                    flow: request.flow,
                    tls: request.tls,
                    reality: request.reality,
                    ws: request.ws,
                    grpc: request.grpc,
                    h2: request.h2,
                    http_upgrade: request.http_upgrade,
                    split_http: request.split_http,
                    quic: request.quic,
                    payload: request.payload,
                },
            )
            .await
    }

    pub(crate) async fn send_vless_relay_two_stream(
        &mut self,
        request: VlessRelayTwoStreamSend<'_>,
    ) -> Result<(), FlowFailure> {
        self.protocol_state
            .start_vless_udp_relay_two_stream(
                &mut self.chain_tasks,
                crate::protocol_runtime::udp::VlessUdpRelayTwoStream {
                    proxy: request.proxy,
                    session: request.session,
                    post_carrier: request.post_carrier,
                    get_carrier: request.get_carrier,
                    uuid: request.uuid,
                    split_http: request.split_http,
                    payload: request.payload,
                },
            )
            .await
    }

    pub(crate) async fn send_vless_relay_final_hop(
        &mut self,
        request: VlessRelayFinalHopSend<'_>,
    ) -> Result<(), FlowFailure> {
        self.protocol_state
            .start_vless_udp_relay_final_hop(
                &mut self.chain_tasks,
                crate::protocol_runtime::udp::VlessUdpRelayFinalHop {
                    proxy: request.proxy,
                    session: request.session,
                    carrier: request.carrier,
                    uuid: request.uuid,
                    tls: request.tls,
                    reality: request.reality,
                    ws: request.ws,
                    grpc: request.grpc,
                    h2: request.h2,
                    http_upgrade: request.http_upgrade,
                    split_http: request.split_http,
                    payload: request.payload,
                },
            )
            .await
    }
}
