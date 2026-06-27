use std::net::SocketAddr;

use tracing::warn;
use zero_core::Address;
use zero_platform_tokio::TokioDatagramSocket;

use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::Proxy;

pub(super) async fn handle_chain_result(
    request: ChainResponseRequest<'_>,
    chain_result: Result<ChainTask, tokio::task::JoinError>,
) {
    match chain_result {
        Ok(Ok((target, port, payload, session_id))) => {
            forward_chain_response(ForwardChainResponseRequest {
                proxy: request.proxy,
                relay: request.relay,
                client_addr: request.client_addr,
                inbound_tag: request.inbound_tag,
                target: &target,
                port,
                payload: &payload,
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
    target: &'a Address,
    port: u16,
    payload: &'a [u8],
    session_id: Option<u64>,
}

async fn forward_chain_response(request: ForwardChainResponseRequest<'_>) {
    if let Some(sid) = request.session_id {
        request
            .proxy
            .record_session_outbound_rx(sid, request.payload.len() as u64);
    }

    let Some(client_addr) = request.client_addr else {
        return;
    };

    let udp_session = socks5::Socks5Inbound.udp_session();
    match udp_session.encode_response_to_client(request.target, request.port, request.payload) {
        Ok(frame) => match request.relay.send_to_addr(&frame, client_addr).await {
            Ok(sent) => {
                if let Some(sid) = request.session_id {
                    request.proxy.record_session_inbound_tx(sid, sent as u64);
                }
            }
            Err(error) => {
                warn!(
                    inbound_tag = request.inbound_tag,
                    protocol = "socks5_udp",
                    ?request.target,
                    port = request.port,
                    error = %error,
                    "failed to send UDP chain response to client"
                );
            }
        },
        Err(error) => {
            warn!(
                inbound_tag = request.inbound_tag,
                protocol = "socks5_udp",
                ?request.target,
                port = request.port,
                error = %error,
                "failed to build SOCKS5 UDP chain response"
            );
        }
    }
}
