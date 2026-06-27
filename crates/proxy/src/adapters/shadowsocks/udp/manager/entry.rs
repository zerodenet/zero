use std::net::SocketAddr;

use super::bridge;
use crate::runtime::udp_flow::managed::{
    ManagedDatagramConnectionCache, ManagedDatagramConnectionCacheKey,
    SharedManagedDatagramUdpConnection,
};
use zero_engine::EngineError;

pub(super) async fn ensure(
    upstreams: &mut ManagedDatagramConnectionCache,
    cache_key: ManagedDatagramConnectionCacheKey,
    resume: &shadowsocks::ShadowsocksUdpFlowResume,
    target_addr: SocketAddr,
) -> Result<SharedManagedDatagramUdpConnection, EngineError> {
    upstreams
        .get_or_insert_with(
            cache_key,
            bridge::establish_datagram_connection(target_addr, resume),
        )
        .await
}
