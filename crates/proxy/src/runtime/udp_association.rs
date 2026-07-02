use std::net::SocketAddr;

use tokio::select;
use zero_engine::EngineError;
use zero_platform_tokio::TokioDatagramSocket;
use zero_traits::AsyncSocket;

use crate::logging::{
    log_udp_upstream_association_dropped, log_udp_upstream_association_idle_timeout,
};
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_flow::helpers::{
    log_completed_udp_flow, record_chain_udp_response_parts, record_upstream_udp_response_received,
    wait_for_upstream_idle, UdpChainResponseParts, UdpUpstreamResponseParts,
};
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::response::UpstreamUdpResponse;
use crate::runtime::udp_response::{write_chain_response, write_upstream_response};
use crate::runtime::Proxy;
use crate::transport::{ClientStream, MeteredStream, StreamTraffic};

pub(crate) trait UdpAssociationHandler {
    async fn handle_client_datagram(
        &mut self,
        proxy: &Proxy,
        dispatch: &mut UdpDispatch,
        relay: &TokioDatagramSocket,
        pending_control_traffic: &mut StreamTraffic,
        sender: SocketAddr,
        payload: &[u8],
    ) -> Result<(), EngineError>;

    async fn write_direct_response(
        &mut self,
        proxy: &Proxy,
        dispatch: &UdpDispatch,
        relay: &TokioDatagramSocket,
        sender: SocketAddr,
        payload: &[u8],
    ) -> Result<(), EngineError>;

    async fn write_upstream_response(
        &mut self,
        relay: &TokioDatagramSocket,
        response: &UdpUpstreamResponseParts<'_>,
    ) -> Result<usize, EngineError>;

    async fn write_chain_response(
        &mut self,
        relay: &TokioDatagramSocket,
        response: &UdpChainResponseParts<'_>,
    ) -> Result<usize, EngineError>;
}

pub(crate) struct UdpAssociationLoopRequest<'a, S, H> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) client: &'a mut MeteredStream<S>,
    pub(crate) inbound_tag: &'a str,
    pub(crate) relay: TokioDatagramSocket,
    pub(crate) pending_control_traffic: StreamTraffic,
    pub(crate) handler: H,
}

pub(crate) async fn run_udp_association_loop<S, H>(
    request: UdpAssociationLoopRequest<'_, S, H>,
) -> Result<(), EngineError>
where
    S: ClientStream,
    H: UdpAssociationHandler,
{
    let UdpAssociationLoopRequest {
        proxy,
        client,
        inbound_tag,
        relay,
        mut pending_control_traffic,
        mut handler,
    } = request;

    let mut dispatch = UdpDispatch::new(inbound_tag).await?;
    let mut control_probe = [0_u8; 1];
    let mut packet = vec![0_u8; 64 * 1024];
    let mut direct_buf = vec![0_u8; 64 * 1024];
    let mut upstream_buf = vec![0_u8; 64 * 1024];

    loop {
        let (direct_sock, upstream_udp, idle_deadline, chain_tasks) = dispatch.poll_refs();

        select! {
            control = client.read(&mut control_probe) => {
                match control {
                    Ok(0) => break,
                    Ok(_) => break,
                    Err(error) => return Err(error.into()),
                }
            }
            recv = relay.recv_from_addr(&mut packet) => {
                let (read, sender) = recv?;
                handler.handle_client_datagram(
                    proxy,
                    &mut dispatch,
                    &relay,
                    &mut pending_control_traffic,
                    sender,
                    &packet[..read],
                )
                .await?;
            }
            recv = direct_sock.recv_from_addr(&mut direct_buf) => {
                let (read, sender) = recv?;
                if let Err(error) = handler
                    .write_direct_response(proxy, &dispatch, &relay, sender, &direct_buf[..read])
                    .await
                {
                    tracing::warn!(
                        inbound_tag = inbound_tag,
                        error = %error,
                        "failed to forward direct UDP response"
                    );
                }
            }
            upstream = upstream_udp.recv_response(&mut upstream_buf) => {
                handle_upstream_response(proxy, &mut dispatch, &mut handler, &relay, inbound_tag, upstream).await?;
            }
            Some(chain_result) = chain_tasks.join_next() => {
                handle_chain_result(proxy, &mut handler, &relay, inbound_tag, chain_result).await;
            }
            _ = wait_for_upstream_idle(idle_deadline) => {
                handle_idle_timeout(proxy, &mut dispatch, inbound_tag);
            }
        }
    }

    finish_dispatch(dispatch);

    Ok(())
}

fn finish_dispatch(dispatch: UdpDispatch) {
    for completed in dispatch.finish_all() {
        log_completed_udp_flow(completed);
    }
}

fn handle_idle_timeout(proxy: &Proxy, dispatch: &mut UdpDispatch, inbound_tag: &str) {
    if let Some(closed) = dispatch.drop_idle_upstream_association() {
        log_udp_upstream_association_idle_timeout(
            inbound_tag,
            &closed.outbound_tag,
            &closed.server,
            closed.port,
            proxy.udp_upstream_idle_timeout(),
        );
    }
}

async fn handle_upstream_response<H>(
    proxy: &Proxy,
    dispatch: &mut UdpDispatch,
    handler: &mut H,
    relay: &TokioDatagramSocket,
    inbound_tag: &str,
    upstream: Result<UpstreamUdpResponse, EngineError>,
) -> Result<(), EngineError>
where
    H: UdpAssociationHandler,
{
    match upstream {
        Ok(response) => {
            let response = record_upstream_udp_response_received(
                proxy,
                dispatch,
                proxy.udp_upstream_idle_timeout(),
                response,
            );
            write_upstream_response(&response, || async {
                handler.write_upstream_response(relay, &response).await
            })
            .await?;
        }
        Err(error) => {
            if let Some(closed) = dispatch.drop_upstream_association() {
                proxy.record_udp_upstream_recv_failure();
                log_udp_upstream_association_dropped(
                    inbound_tag,
                    &closed.outbound_tag,
                    &closed.server,
                    closed.port,
                    &error,
                );
            }
        }
    }

    Ok(())
}

async fn handle_chain_result<H>(
    proxy: &Proxy,
    handler: &mut H,
    relay: &TokioDatagramSocket,
    inbound_tag: &str,
    chain_result: Result<ChainTask, tokio::task::JoinError>,
) where
    H: UdpAssociationHandler,
{
    match chain_result {
        Ok(Ok((target, port, payload, session_id))) => {
            let response =
                record_chain_udp_response_parts(proxy, target, port, payload, session_id);
            if let Err(error) = write_chain_response(&response, || async {
                handler.write_chain_response(relay, &response).await
            })
            .await
            {
                tracing::warn!(
                    inbound_tag = inbound_tag,
                    target = ?response.target,
                    port = response.port,
                    error = ?error,
                    "failed to send UDP chain response to client"
                );
            }
        }
        Ok(Err(error)) => {
            tracing::warn!(error = %error, "chain upstream read error");
        }
        Err(join_err) => {
            tracing::warn!(error = %join_err, "chain response task panicked");
        }
    }
}
