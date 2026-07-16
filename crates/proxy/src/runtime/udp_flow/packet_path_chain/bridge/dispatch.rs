use tokio::sync::oneshot;
use zero_engine::EngineError;

use super::super::model::Entry;
use super::waiter::{remove_waiter, Waiter};
use crate::runtime::udp_flow::packet_path::{UdpFlowContext, UdpPacketRef};
use crate::runtime::udp_flow::result::FlowFailure;

/// Encode + send + spawn the response bridge. Shared by [`PacketPathManager::send`]
/// (start) and [`PacketPathManager::send_with_snapshot`] (forward).
pub(in crate::runtime::udp_flow::packet_path_chain) async fn dispatch_via_entry(
    entry: &Entry,
    ctx: UdpFlowContext<'_>,
    packet_ref: UdpPacketRef<'_>,
) -> Result<usize, FlowFailure> {
    let codec = entry.codec.clone();
    let packet = codec
        .encode(packet_ref.target, packet_ref.port, packet_ref.payload)
        .map_err(|error| FlowFailure {
            stage: "packet_path_encode",
            error: error.into(),
            upstream: Some(entry.datagram_endpoint.upstream()),
        })?;

    let (response_tx, response_rx) = oneshot::channel();
    entry
        .waiters
        .lock()
        .expect("packet path waiters lock poisoned")
        .push_back(Waiter {
            target: packet_ref.target.clone(),
            port: packet_ref.port,
            tx: response_tx,
        });

    let datagram_target = entry.datagram_endpoint.target();
    let datagram_port = entry.datagram_endpoint.port();
    if let Err(error) = entry
        .path
        .send_to(&datagram_target, datagram_port, &packet)
        .await
    {
        remove_waiter(&entry.waiters, packet_ref.target, packet_ref.port);
        return Err(FlowFailure {
            stage: "packet_path_send",
            error,
            upstream: Some(entry.datagram_endpoint.upstream()),
        });
    }

    ctx.chain_tasks.spawn(async move {
        match response_rx.await {
            Ok((target, port, payload)) => Ok((target, port, payload, Some(ctx.session_id))),
            Err(_) => Err(EngineError::Io(std::io::Error::other(
                "packet path upstream closed",
            ))),
        }
    });

    Ok(packet_ref.payload.len())
}
