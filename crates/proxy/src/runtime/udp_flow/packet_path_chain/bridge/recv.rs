use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use tracing::{debug, warn};
use zero_core::Address;

use super::waiter::{remove_waiter, Waiter};
use crate::runtime::udp_flow::packet_path::{DatagramCodec, PacketPathCarrier};

pub(in crate::runtime::udp_flow::packet_path_chain) async fn recv_loop(
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
