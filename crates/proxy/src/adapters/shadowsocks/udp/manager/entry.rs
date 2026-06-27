use std::net::SocketAddr;
use std::sync::Arc;

use super::bridge::{self, BridgeWaiters};
use super::model::SsUpstream;
use zero_engine::EngineError;
use zero_transport::shadowsocks_transport;

pub(super) async fn ensure(
    upstreams: &mut shadowsocks::ShadowsocksUdpFlowStore<Arc<SsUpstream>>,
    resume: &shadowsocks::ShadowsocksUdpFlowResume,
    target_addr: SocketAddr,
) -> Result<Arc<SsUpstream>, EngineError> {
    if let Some(entry) = upstreams.get(resume) {
        return Ok(entry.clone());
    }

    let flow = Arc::new(
        shadowsocks_transport::establish_shadowsocks_udp_socket_flow(
            target_addr,
            Arc::new(resume.socket_flow_codec()),
        )
        .await?,
    );
    let waiters = BridgeWaiters::new();
    let entry = Arc::new(SsUpstream {
        flow: flow.clone(),
        waiters,
    });
    upstreams.insert(resume, entry.clone());

    bridge::spawn_upstream_response_pump(flow, entry.waiters.clone_handle());
    Ok(entry)
}
