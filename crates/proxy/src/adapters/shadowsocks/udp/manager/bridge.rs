use std::sync::Arc;

use zero_core::Address;
use zero_engine::EngineError;
use zero_transport::shadowsocks_transport::{self, ShadowsocksUdpSocketFlow};

use crate::runtime::udp_flow::managed::{
    managed_datagram_connection, ManagedDatagramSender, SharedManagedDatagramUdpConnection,
};

struct ShadowsocksDatagramSender {
    flow: Arc<ShadowsocksUdpSocketFlow>,
}

#[async_trait::async_trait]
impl ManagedDatagramSender for ShadowsocksDatagramSender {
    async fn send_datagram(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<(), EngineError> {
        self.flow.send_datagram(target, port, payload).await
    }
}

pub(super) async fn establish_datagram_connection(
    target_addr: std::net::SocketAddr,
    resume: &shadowsocks::ShadowsocksUdpFlowResume,
) -> Result<SharedManagedDatagramUdpConnection, EngineError> {
    let flow = Arc::new(
        shadowsocks_transport::establish_shadowsocks_udp_socket_flow(
            target_addr,
            Arc::new(resume.socket_flow_codec()),
        )
        .await?,
    );
    Ok(managed_datagram_connection(
        Arc::new(ShadowsocksDatagramSender { flow: flow.clone() }),
        flow.subscribe(),
        "ss upstream closed",
    ))
}
