use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use super::bridge::BridgeWaiters;
use super::model::{SsKey, SsUpstream};
use zero_engine::EngineError;
use zero_transport::shadowsocks_transport;

pub(super) async fn ensure(
    upstreams: &mut HashMap<SsKey, Arc<SsUpstream>>,
    leaf_key: shadowsocks::ShadowsocksUdpLeafKey,
    resume: shadowsocks::ShadowsocksUdpFlowResume,
    target_addr: SocketAddr,
) -> Result<Arc<SsUpstream>, EngineError> {
    let key = SsKey::new(leaf_key);
    if let Some(entry) = upstreams.get(&key) {
        return Ok(entry.clone());
    }

    let flow = Arc::new(
        shadowsocks_transport::establish_shadowsocks_udp_socket_flow(target_addr, resume).await?,
    );
    let waiters = BridgeWaiters::new();
    let entry = Arc::new(SsUpstream {
        flow: flow.clone(),
        waiters,
    });
    upstreams.insert(key, entry.clone());

    bridge_responses(flow, entry.waiters.clone_handle());
    Ok(entry)
}

fn bridge_responses(
    flow: Arc<shadowsocks_transport::ShadowsocksUdpSocketFlow>,
    waiters: BridgeWaiters,
) {
    tokio::spawn(async move {
        let mut recv_rx = flow.subscribe();
        while let Ok((target, port, payload)) = recv_rx.recv().await {
            waiters.deliver(target, port, payload);
        }
    });
}
