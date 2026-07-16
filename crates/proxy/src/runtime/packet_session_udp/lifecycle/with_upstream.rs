use tokio::select;
use tokio::time::Instant as TokioInstant;
use tracing::warn;
use zero_engine::EngineError;

use super::failure::handle_runtime_failure;
use super::relay::PacketSessionUdpLoopContext;
use crate::runtime::packet_session_udp::contract::{
    PacketSessionUdpHandler, PacketSessionUdpReadFailureAction, PacketSessionUdpReadResult,
};
use crate::runtime::udp_delivery::write_upstream_response;
use crate::runtime::udp_delivery::{
    record_chain_udp_response_parts, record_direct_udp_response_parts, write_chain_response,
    write_direct_response,
};
use crate::runtime::udp_delivery::{record_upstream_udp_response_received, wait_for_upstream_idle};

pub(super) async fn run_loop<H>(
    context: &PacketSessionUdpLoopContext<'_>,
    handler: &mut H,
    dispatch: &mut crate::runtime::udp_dispatch::UdpDispatch,
    last_activity: &mut TokioInstant,
    direct_buf: &mut [u8],
    upstream_buf: &mut [u8],
) -> Result<(), EngineError>
where
    H: PacketSessionUdpHandler,
{
    loop {
        let (direct_sock, upstream_udp, upstream_idle_deadline, chain_tasks) = dispatch.poll_refs();

        select! {
            _ = tokio::time::sleep_until(*last_activity + context.timeout) => {
                tracing::info!(
                    inbound_tag = context.inbound_tag,
                    protocol = context.protocol,
                    "packet session udp relay idle timeout"
                );
                break;
            }
            read = handler.read_inbound_dispatch() => {
                match read {
                    Ok(PacketSessionUdpReadResult::Dispatch(inbound_dispatch)) => {
                        *last_activity = TokioInstant::now();
                        if let Err(error) = context
                            .runtime
                            .dispatch_inbound_packet(dispatch, &inbound_dispatch, context.auth)
                            .await
                        {
                            warn!(error = %error, protocol = context.protocol, "packet session udp dispatch failed");
                        }
                    }
                    Ok(PacketSessionUdpReadResult::End) => break,
                    Err(failure) => {
                        warn!(
                            error = %failure.error,
                            protocol = context.protocol,
                            "packet session udp inbound read/decode error"
                        );
                        match failure.action {
                            #[cfg(any(feature = "vless", feature = "vmess"))]
                            PacketSessionUdpReadFailureAction::Continue => continue,
                            PacketSessionUdpReadFailureAction::End => break,
                        }
                    }
                }
            }
            recv = direct_sock.recv_from_addr(direct_buf) => {
                match recv {
                    Ok((n, sender)) => {
                        *last_activity = TokioInstant::now();
                        let response = record_direct_udp_response_parts(
                            context.runtime.services(),
                            dispatch,
                            sender,
                            &direct_buf[..n],
                        );
                        if let Err(error) = write_direct_response(&response, || async {
                            handler
                                .write_response_for_target(
                                    &response.target,
                                    response.port,
                                    response.payload,
                                )
                                .await
                        })
                        .await
                        {
                            return handle_runtime_failure(
                                handler,
                                context.failure_policy,
                                context.inbound_tag,
                                context.protocol,
                                "packet session udp direct response encode failed",
                                error.into(),
                            )
                            .await;
                        }
                    }
                    Err(error) => {
                        return handle_runtime_failure(
                            handler,
                            context.failure_policy,
                            context.inbound_tag,
                            context.protocol,
                            "packet session udp direct recv failed",
                            error.into(),
                        )
                        .await;
                    }
                }
            }
            upstream = upstream_udp.recv_response(upstream_buf) => {
                match upstream {
                    Ok(pkt) => {
                        *last_activity = TokioInstant::now();
                        let response = record_upstream_udp_response_received(
                            context.runtime.services(),
                            dispatch,
                            context.timeout,
                            pkt,
                        );
                        if let Err(error) = write_upstream_response(&response, || async {
                            handler
                                .write_response_for_target(
                                    &response.target,
                                    response.port,
                                    &response.payload,
                                )
                                .await
                        })
                        .await
                        {
                            return handle_runtime_failure(
                                handler,
                                context.failure_policy,
                                context.inbound_tag,
                                context.protocol,
                                "packet session udp upstream response encode failed",
                                error.into(),
                            )
                            .await;
                        }
                    }
                    Err(error) => warn!(error = %error, protocol = context.protocol, "packet session udp upstream recv failed"),
                }
            }
            _ = wait_for_upstream_idle(upstream_idle_deadline) => {}
            Some(chain_result) = chain_tasks.join_next() => {
                match chain_result {
                    Ok(Ok((target, port, payload, session_id))) => {
                        *last_activity = TokioInstant::now();
                        let response = record_chain_udp_response_parts(
                            context.runtime.services(),
                            target,
                            port,
                            payload,
                            session_id,
                        );
                        if let Err(error) = write_chain_response(&response, || async {
                            handler
                                .write_response_for_target(
                                    &response.target,
                                    response.port,
                                    &response.payload,
                                )
                                .await
                        })
                        .await
                        {
                            return handle_runtime_failure(
                                handler,
                                context.failure_policy,
                                context.inbound_tag,
                                context.protocol,
                                "packet session udp chain response encode failed",
                                error.into(),
                            )
                            .await;
                        }
                    }
                    Ok(Err(error)) => warn!(error = %error, protocol = context.protocol, "packet session udp chain response failed"),
                    Err(error) => warn!(error = %error, protocol = context.protocol, "packet session udp chain task panicked"),
                }
            }
        }
    }

    Ok(())
}
