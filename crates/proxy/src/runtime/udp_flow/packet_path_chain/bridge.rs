use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use tokio::sync::oneshot;
use tracing::{debug, warn};
use zero_core::Address;
use zero_engine::EngineError;

use super::Entry;
use crate::runtime::udp_flow::packet_path::{
    DatagramCodec, PacketPathCarrier, UdpFlowContext, UdpPacketRef,
};
use crate::runtime::udp_flow::result::FlowFailure;

type RecvItem = (Address, u16, Vec<u8>);

pub(super) struct Waiter {
    target: Address,
    port: u16,
    tx: oneshot::Sender<RecvItem>,
}

/// Encode + send + spawn the response bridge. Shared by [`PacketPathManager::send`]
/// (start) and [`PacketPathManager::send_with_snapshot`] (forward).
pub(super) async fn dispatch_via_entry(
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

pub(super) async fn recv_loop(
    path: Arc<dyn PacketPathCarrier>,
    waiters: Arc<Mutex<VecDeque<Waiter>>>,
    codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
) {
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let read = match path.recv_from(&mut buf).await {
            Ok(read) => read,
            Err(error) => {
                warn!(error = %error, "packet path recv loop stopped");
                break;
            }
        };
        let decoded = match codec.decode(&buf[..read]) {
            Some(d) => d,
            None => {
                warn!(bytes = read, "failed to decode inner datagram response");
                continue;
            }
        };
        debug!(
            target = ?decoded.0,
            port = decoded.1,
            bytes = decoded.2.len(),
            "decoded packet path datagram response"
        );
        if let Some(waiter) = remove_waiter(&waiters, &decoded.0, decoded.1) {
            let _ = waiter.tx.send(decoded);
        } else {
            warn!(
                target = ?decoded.0,
                port = decoded.1,
                "no waiter for packet path datagram response"
            );
        }
    }
}

fn remove_waiter(waiters: &Mutex<VecDeque<Waiter>>, target: &Address, port: u16) -> Option<Waiter> {
    let mut waiters = waiters.lock().expect("packet path waiters lock poisoned");
    let index = waiters
        .iter()
        .position(|waiter| waiter.target == *target && waiter.port == port)?;
    waiters.remove(index)
}
