use std::net::SocketAddr;

use tracing::warn;
use zero_platform_tokio::TokioDatagramSocket;

use crate::runtime::udp_flow::helpers::record_chain_udp_response_received;
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::Proxy;

pub(super) async fn handle_chain_result(
    request: ChainResponseRequest<'_>,
    chain_result: Result<ChainTask, tokio::task::JoinError>,
) {
    match chain_result {
        Ok(Ok((target, port, payload, session_id))) => {
            let response = socks5::udp::Socks5UdpClientResponse::new(&target, port, &payload);
            forward_chain_response(ForwardChainResponseRequest {
                proxy: request.proxy,
                relay: request.relay,
                client_addr: request.client_addr,
                inbound_tag: request.inbound_tag,
                response,
                session_id,
            })
            .await;
        }
        Ok(Err(error)) => {
            tracing::warn!(error = %error, "chain upstream read error");
        }
        Err(join_err) => {
            tracing::warn!(error = %join_err, "chain response task panicked");
        }
    }
}

pub(super) struct ChainResponseRequest<'a> {
    pub proxy: &'a Proxy,
    pub relay: &'a TokioDatagramSocket,
    pub client_addr: Option<SocketAddr>,
    pub inbound_tag: &'a str,
}

struct ForwardChainResponseRequest<'a> {
    proxy: &'a Proxy,
    relay: &'a TokioDatagramSocket,
    client_addr: Option<SocketAddr>,
    inbound_tag: &'a str,
    response: socks5::udp::Socks5UdpClientResponse<'a>,
    session_id: Option<u64>,
}

async fn forward_chain_response(request: ForwardChainResponseRequest<'_>) {
    let response_accounting = record_chain_udp_response_received(
        request.proxy,
        request.session_id,
        request.response.payload_len(),
    );

    let Some(client_addr) = request.client_addr else {
        return;
    };

    let udp_session = socks5::Socks5Inbound.udp_session();
    match udp_session
        .send_client_response(
            request.relay,
            zero_platform_tokio::socket_addr_to_socket_address(client_addr),
            request.response,
        )
        .await
    {
        Ok(sent) => {
            response_accounting.record_sent(sent);
        }
        Err(error) => {
            warn!(
                inbound_tag = request.inbound_tag,
                protocol = "socks5_udp",
                ?request.response,
                error = ?error,
                "failed to send SOCKS5 UDP chain response to client"
            );
        }
    }
}
