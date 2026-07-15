use tokio::select;
use tokio::time::Instant as TokioInstant;
use tracing::{info, warn};
use zero_engine::EngineError;

use super::contract::{
    PacketSessionUdpFailurePolicy, PacketSessionUdpHandler, PacketSessionUdpReadFailureAction,
    PacketSessionUdpReadResult, PacketSessionUdpRelayRequest,
};
use crate::protocol_registry::UdpRuntimeServices;
#[cfg(feature = "socks5")]
use crate::runtime::udp_delivery::write_upstream_response;
use crate::runtime::udp_delivery::{
    log_completed_udp_flow, record_chain_udp_response_parts, record_direct_udp_response_parts,
};
#[cfg(feature = "socks5")]
use crate::runtime::udp_delivery::{record_upstream_udp_response_received, wait_for_upstream_idle};
use crate::runtime::udp_delivery::{write_chain_response, write_direct_response};
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_ingress::dispatch_inbound_udp_packet;
use crate::runtime::Proxy;

pub(crate) async fn run_packet_session_udp_relay<H>(
    proxy: &Proxy,
    request: PacketSessionUdpRelayRequest<'_, H>,
) -> Result<(), EngineError>
where
    H: PacketSessionUdpHandler,
{
    let PacketSessionUdpRelayRequest {
        mut handler,
        inbound_tag,
        protocol,
        auth,
        failure_policy,
    } = request;

    let mut dispatch = match UdpDispatch::new(inbound_tag, &proxy.protocols).await {
        Ok(dispatch) => dispatch,
        Err(error) => {
            return handle_runtime_failure(
                &mut handler,
                failure_policy,
                inbound_tag,
                protocol,
                "packet session udp dispatch init failed",
                error,
            )
            .await;
        }
    };
    let services = UdpRuntimeServices::from_proxy(proxy);

    let timeout = services.udp_upstream_idle_timeout();
    let mut last_activity = TokioInstant::now();
    let mut direct_buf = vec![0_u8; 64 * 1024];
    #[cfg(feature = "socks5")]
    let mut upstream_buf = vec![0_u8; 64 * 1024];

    info!(
        inbound_tag = inbound_tag,
        protocol = protocol,
        "packet session udp relay started"
    );

    #[cfg(feature = "socks5")]
    loop {
        let (direct_sock, upstream_udp, upstream_idle_deadline, chain_tasks) = dispatch.poll_refs();

        select! {
            _ = tokio::time::sleep_until(last_activity + timeout) => {
                info!(
                    inbound_tag = inbound_tag,
                    protocol = protocol,
                    "packet session udp relay idle timeout"
                );
                break;
            }
            read = handler.read_inbound_dispatch() => {
                match read {
                    Ok(PacketSessionUdpReadResult::Dispatch(inbound_dispatch)) => {
                        last_activity = TokioInstant::now();
                        if let Err(error) = dispatch_inbound_udp_packet(
                            proxy,
                            &mut dispatch,
                            &inbound_dispatch,
                            auth.as_ref(),
                        )
                        .await
                        {
                            warn!(error = %error, protocol = protocol, "packet session udp dispatch failed");
                        }
                    }
                    Ok(PacketSessionUdpReadResult::End) => break,
                    Err(failure) => {
                        warn!(
                            error = %failure.error,
                            protocol = protocol,
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
            recv = direct_sock.recv_from_addr(&mut direct_buf) => {
                match recv {
                    Ok((n, sender)) => {
                        last_activity = TokioInstant::now();
                        let response = record_direct_udp_response_parts(
                            &services,
                            &dispatch,
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
                                &mut handler,
                                failure_policy,
                                inbound_tag,
                                protocol,
                                "packet session udp direct response encode failed",
                                error.into(),
                            )
                            .await;
                        }
                    }
                    Err(error) => {
                        return handle_runtime_failure(
                            &mut handler,
                            failure_policy,
                            inbound_tag,
                            protocol,
                            "packet session udp direct recv failed",
                            error.into(),
                        )
                        .await;
                    }
                }
            }
            upstream = upstream_udp.recv_response(&mut upstream_buf) => {
                match upstream {
                    Ok(pkt) => {
                        last_activity = TokioInstant::now();
                        let response = record_upstream_udp_response_received(
                            &services,
                            &mut dispatch,
                            timeout,
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
                                &mut handler,
                                failure_policy,
                                inbound_tag,
                                protocol,
                                "packet session udp upstream response encode failed",
                                error.into(),
                            )
                            .await;
                        }
                    }
                    Err(error) => warn!(error = %error, protocol = protocol, "packet session udp upstream recv failed"),
                }
            }
            _ = wait_for_upstream_idle(upstream_idle_deadline) => {}
            Some(chain_result) = chain_tasks.join_next() => {
                match chain_result {
                    Ok(Ok((target, port, payload, session_id))) => {
                        last_activity = TokioInstant::now();
                        let response =
                            record_chain_udp_response_parts(&services, target, port, payload, session_id);
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
                                &mut handler,
                                failure_policy,
                                inbound_tag,
                                protocol,
                                "packet session udp chain response encode failed",
                                error.into(),
                            )
                            .await;
                        }
                    }
                    Ok(Err(error)) => warn!(error = %error, protocol = protocol, "packet session udp chain response failed"),
                    Err(error) => warn!(error = %error, protocol = protocol, "packet session udp chain task panicked"),
                }
            }
        }
    }

    #[cfg(not(feature = "socks5"))]
    loop {
        let (direct_sock, chain_tasks) = dispatch.poll_sockets();

        select! {
            _ = tokio::time::sleep_until(last_activity + timeout) => {
                info!(inbound_tag = inbound_tag, protocol = protocol, "packet session udp relay idle timeout");
                break;
            }
            read = handler.read_inbound_dispatch() => {
                match read {
                    Ok(PacketSessionUdpReadResult::Dispatch(inbound_dispatch)) => {
                        last_activity = TokioInstant::now();
                        if let Err(error) = dispatch_inbound_udp_packet(
                            proxy,
                            &mut dispatch,
                            &inbound_dispatch,
                            auth.as_ref(),
                        ).await {
                            warn!(error = %error, protocol = protocol, "packet session udp dispatch failed");
                        }
                    }
                    Ok(PacketSessionUdpReadResult::End) => break,
                    Err(failure) => {
                        warn!(error = %failure.error, protocol = protocol, "packet session udp inbound read/decode error");
                        match failure.action {
                            #[cfg(any(feature = "vless", feature = "vmess"))]
                            PacketSessionUdpReadFailureAction::Continue => continue,
                            PacketSessionUdpReadFailureAction::End => break,
                        }
                    }
                }
            }
            recv = direct_sock.recv_from_addr(&mut direct_buf) => {
                match recv {
                    Ok((n, sender)) => {
                        last_activity = TokioInstant::now();
                        let response = record_direct_udp_response_parts(&services, &dispatch, sender, &direct_buf[..n]);
                        if let Err(error) = write_direct_response(&response, || async {
                            handler.write_response_for_target(&response.target, response.port, response.payload).await
                        }).await {
                            return handle_runtime_failure(
                                &mut handler, failure_policy, inbound_tag, protocol,
                                "packet session udp direct response encode failed", error.into(),
                            ).await;
                        }
                    }
                    Err(error) => {
                        return handle_runtime_failure(
                            &mut handler, failure_policy, inbound_tag, protocol,
                            "packet session udp direct recv failed", error.into(),
                        ).await;
                    }
                }
            }
            Some(chain_result) = chain_tasks.join_next() => {
                match chain_result {
                    Ok(Ok((target, port, payload, session_id))) => {
                        last_activity = TokioInstant::now();
                        let response = record_chain_udp_response_parts(&services, target, port, payload, session_id);
                        if let Err(error) = write_chain_response(&response, || async {
                            handler.write_response_for_target(&response.target, response.port, &response.payload).await
                        }).await {
                            return handle_runtime_failure(
                                &mut handler, failure_policy, inbound_tag, protocol,
                                "packet session udp chain response encode failed", error.into(),
                            ).await;
                        }
                    }
                    Ok(Err(error)) => warn!(error = %error, protocol = protocol, "packet session udp chain response failed"),
                    Err(error) => warn!(error = %error, protocol = protocol, "packet session udp chain task panicked"),
                }
            }
        }
    }

    for completed in dispatch.finish_all() {
        log_completed_udp_flow(completed);
    }

    let _ = handler.finish().await;

    info!(
        inbound_tag = inbound_tag,
        protocol = protocol,
        "packet session udp relay ended"
    );

    Ok(())
}

async fn handle_runtime_failure<H>(
    _handler: &mut H,
    failure_policy: PacketSessionUdpFailurePolicy,
    _inbound_tag: &str,
    _protocol: &'static str,
    _message: &'static str,
    error: EngineError,
) -> Result<(), EngineError>
where
    H: PacketSessionUdpHandler,
{
    match failure_policy {
        PacketSessionUdpFailurePolicy::ReturnError => Err(error),
        #[cfg(any(feature = "vless", feature = "vmess"))]
        PacketSessionUdpFailurePolicy::LogAndBreak => {
            warn!(
                inbound_tag = _inbound_tag,
                protocol = _protocol,
                error = %error,
                "{_message}"
            );
            let _ = _handler.finish().await;
            Ok(())
        }
    }
}
