use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use super::bridge::BridgeWaiters;
use super::model::{SsKey, SsUpstream};
use super::socket;

pub(super) fn ensure(
    upstreams: &mut HashMap<SsKey, Arc<SsUpstream>>,
    leaf_key: shadowsocks::ShadowsocksUdpLeafKey,
    resume: shadowsocks::ShadowsocksUdpFlowResume,
    target_addr: SocketAddr,
) -> Arc<SsUpstream> {
    let key = SsKey::new(leaf_key);
    if let Some(entry) = upstreams.get(&key) {
        return entry.clone();
    }

    let socket = socket::bind_for_target(target_addr);
    let waiters = BridgeWaiters::new();
    let entry = Arc::new(SsUpstream {
        socket: socket.clone(),
        waiters,
        resume,
    });
    upstreams.insert(key, entry.clone());

    socket::spawn_recv_loop(socket, entry.resume.clone(), entry.waiters.clone_handle());
    entry
}
