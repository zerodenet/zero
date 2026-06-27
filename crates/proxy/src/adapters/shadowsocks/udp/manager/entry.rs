use std::net::SocketAddr;
use std::sync::Arc;

use super::bridge::{self, BridgeWaiters};
use super::model::SsUpstream;
use crate::runtime::udp_flow::managed::SharedManagedDatagramUdpConnection;
use zero_engine::EngineError;
use zero_transport::shadowsocks_transport;

pub(super) async fn ensure(
    upstreams: &mut shadowsocks::ShadowsocksUdpFlowEntries<SharedManagedDatagramUdpConnection>,
    resume: &shadowsocks::ShadowsocksUdpFlowResume,
    target_addr: SocketAddr,
) -> Result<SharedManagedDatagramUdpConnection, EngineError> {
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
    let response_waiters = waiters.clone_handle();
    let entry: SharedManagedDatagramUdpConnection = Arc::new(SsUpstream {
        flow: flow.clone(),
        waiters,
    });
    upstreams.insert(resume, entry.clone());

    bridge::spawn_upstream_response_pump(flow, response_waiters);
    Ok(entry)
}
