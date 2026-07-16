use tokio::select;
use zero_core::DatagramUdpResponder;
use zero_engine::EngineError;

use super::read::process_datagram_read;
use super::relay::DatagramUdpLoopContext;
use super::response::{handle_chain_result, handle_direct_response};
use crate::runtime::udp_dispatch::UdpDispatch;

pub(super) async fn run_loop<S, R>(
    context: &DatagramUdpLoopContext<'_>,
    source: &S,
    responder: &mut R,
    dispatch: &mut UdpDispatch,
    direct_buf: &mut [u8],
) -> Result<(), EngineError>
where
    S: Send,
    R: DatagramUdpResponder<S>,
{
    loop {
        let (direct_sock, chain_tasks) = dispatch.poll_sockets();
        select! {
            read = responder.read_inbound_dispatch(source) => {
                if !process_datagram_read::<S, R>(context, dispatch, responder, read).await {
                    break;
                }
            }
            recv = direct_sock.recv_from_addr(direct_buf) => {
                let (n, sender) = recv?;
                handle_direct_response(context, source, responder, dispatch, sender, &direct_buf[..n]).await;
            }
            Some(chain_result) = chain_tasks.join_next() => {
                handle_chain_result(context, source, responder, chain_result).await;
            }
        }
    }

    Ok(())
}
