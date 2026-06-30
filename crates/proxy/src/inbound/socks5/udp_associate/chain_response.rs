use std::net::SocketAddr;

use tracing::warn;
use zero_platform_tokio::TokioDatagramSocket;

use super::protocol_glue;
use crate::inbound::udp_response::write_chain_response;
use crate::runtime::udp_flow::helpers::{record_chain_udp_response_parts, UdpChainResponseParts};
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::Proxy;

pub(super) async fn handle_chain_result(
    request: ChainResponseRequest<'_>,
    chain_result: Result<ChainTask, tokio::task::JoinError>,
) {
    match chain_result {
        Ok(Ok((target, port, payload, session_id))) => {
            let response =
                record_chain_udp_response_parts(request.proxy, target, port, payload, session_id);
            forward_chain_response(ForwardChainResponseRequest {
                relay: request.relay,
                client_addr: request.client_addr,
                inbound_tag: request.inbound_tag,
                response,
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
    relay: &'a TokioDatagramSocket,
    client_addr: Option<SocketAddr>,
    inbound_tag: &'a str,
    response: UdpChainResponseParts<'a>,
}

async fn forward_chain_response(request: ForwardChainResponseRequest<'_>) {
    let Some(client_addr) = request.client_addr else {
        return;
    };

    match write_chain_response(&request.response, || async {
        protocol_glue::send_client_response_for_target(
            request.relay,
            client_addr,
            &request.response.target,
            request.response.port,
            &request.response.payload,
        )
        .await
    })
    .await
    {
        Ok(_) => {}
        Err(error) => {
            warn!(
                inbound_tag = request.inbound_tag,
                protocol = "socks5_udp",
                target = ?request.response.target,
                port = request.response.port,
                error = ?error,
                "failed to send SOCKS5 UDP chain response to client"
            );
        }
    }
}
