use std::net::SocketAddr;

use super::bridge;
use crate::runtime::udp_flow::managed::{
    ManagedDatagramConnectionCache, SharedManagedDatagramUdpConnection,
};
use zero_engine::EngineError;

pub(super) async fn ensure(
    upstreams: &mut ManagedDatagramConnectionCache,
    cache_key: String,
    resume: &shadowsocks::ShadowsocksUdpFlowResume,
    target_addr: SocketAddr,
) -> Result<SharedManagedDatagramUdpConnection, EngineError> {
    upstreams
        .get_or_insert_key(
            cache_key,
            bridge::establish_datagram_connection(target_addr, resume),
        )
        .await
}
