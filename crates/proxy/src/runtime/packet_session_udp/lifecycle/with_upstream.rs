use tokio::select;
use tokio::time::Instant as TokioInstant;
use zero_engine::EngineError;

use super::failure::handle_runtime_failure;
use super::read::process_packet_session_read;
use super::relay::PacketSessionUdpLoopContext;
use super::response::{handle_chain_result, handle_direct_response, handle_upstream_response};
use crate::runtime::packet_session_udp::contract::PacketSessionUdpHandler;
use crate::runtime::udp_delivery::wait_for_upstream_idle;

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
                if !process_packet_session_read(context, dispatch, last_activity, read).await {
                    break;
                }
            }
            recv = direct_sock.recv_from_addr(direct_buf) => {
                match recv {
                    Ok((n, sender)) => {
                        handle_direct_response(
                            context,
                            handler,
                            dispatch,
                            last_activity,
                            sender,
                            &direct_buf[..n],
                        )
                        .await?;
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
                handle_upstream_response(context, handler, dispatch, last_activity, upstream).await?;
            }
            _ = wait_for_upstream_idle(upstream_idle_deadline) => {}
            Some(chain_result) = chain_tasks.join_next() => {
                handle_chain_result(context, handler, last_activity, chain_result).await?;
            }
        }
    }

    Ok(())
}
