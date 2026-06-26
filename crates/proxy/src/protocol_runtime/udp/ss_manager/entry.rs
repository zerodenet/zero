use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use super::bridge::BridgeWaiters;
use super::model::{SsKey, SsUpstream};
use super::socket;
use zero_core::{Address, Error};
use zero_traits::DatagramCodec;

pub(super) fn ensure(
    upstreams: &mut HashMap<SsKey, Arc<SsUpstream>>,
    server: &str,
    port: u16,
    cache_key: &str,
    codec: Arc<dyn DatagramCodec<Address, Error = Error>>,
    target_addr: SocketAddr,
) -> Arc<SsUpstream> {
    let key = SsKey::new(server, port, cache_key);
    if let Some(entry) = upstreams.get(&key) {
        return entry.clone();
    }

    let socket = socket::bind_for_target(target_addr);
    let waiters = BridgeWaiters::new();
    let entry = Arc::new(SsUpstream {
        socket: socket.clone(),
        waiters,
        codec: codec.clone(),
    });
    upstreams.insert(key, entry.clone());

    socket::spawn_recv_loop(socket, codec, entry.waiters.clone_handle());
    entry
}
