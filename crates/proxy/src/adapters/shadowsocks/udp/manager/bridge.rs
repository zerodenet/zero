use std::sync::Arc;

use tokio::task::JoinSet;
use zero_core::Address;
use zero_engine::EngineError;
use zero_transport::shadowsocks_transport::{self, ShadowsocksUdpSocketFlow};

use crate::runtime::udp_flow::managed::{
    spawn_datagram_response_bridge, ManagedDatagramResponseWaiters, ManagedDatagramUdpConnection,
    SharedManagedDatagramUdpConnection,
};
use crate::runtime::udp_flow::packet_path::ChainTask;

struct SsDatagramConnection {
    flow: Arc<ShadowsocksUdpSocketFlow>,
    waiters: ManagedDatagramResponseWaiters,
}

#[async_trait::async_trait]
impl ManagedDatagramUdpConnection for SsDatagramConnection {
    async fn send_datagram(
        &self,
        chain_tasks: &mut JoinSet<ChainTask>,
        session_id: u64,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        let response_rx = self.waiters.register(target, port);
        if let Err(error) = self.flow.send_datagram(target, port, payload).await {
            self.waiters.remove(target, port);
            return Err(error);
        }

        spawn_datagram_response_bridge(chain_tasks, response_rx, session_id, "ss upstream closed");
        Ok(payload.len())
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
    let waiters = ManagedDatagramResponseWaiters::new();
    let response_waiters = waiters.clone_handle();
    let connection: SharedManagedDatagramUdpConnection = Arc::new(SsDatagramConnection {
        flow: flow.clone(),
        waiters,
    });
    spawn_upstream_response_pump(flow, response_waiters);
    Ok(connection)
}

pub(super) fn spawn_upstream_response_pump(
    flow: Arc<ShadowsocksUdpSocketFlow>,
    waiters: ManagedDatagramResponseWaiters,
) {
    tokio::spawn(async move {
        let mut recv_rx = flow.subscribe();
        while let Ok((target, port, payload)) = recv_rx.recv().await {
            waiters.deliver(target, port, payload);
        }
    });
}
