use std::sync::Arc;

use zero_core::Address;
use zero_engine::EngineError;
use zero_transport::shadowsocks_transport::ShadowsocksUdpSocketFlow;

use super::bridge::BridgeWaiters;
use crate::runtime::udp_flow::managed::ManagedDatagramUdpConnection;
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::Proxy;

pub(super) struct SsUpstream {
    pub(super) flow: Arc<ShadowsocksUdpSocketFlow>,
    pub(super) waiters: BridgeWaiters,
}

#[async_trait::async_trait]
impl ManagedDatagramUdpConnection for SsUpstream {
    async fn send_datagram(
        &self,
        chain_tasks: &mut tokio::task::JoinSet<ChainTask>,
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

        super::bridge::spawn_response_bridge(chain_tasks, response_rx, session_id);
        Ok(payload.len())
    }
}

pub(super) struct SsSendExisting<'a> {
    pub(super) chain_tasks:
        &'a mut tokio::task::JoinSet<crate::runtime::udp_flow::packet_path::ChainTask>,
    pub(super) session_id: u64,
    pub(super) proxy: &'a Proxy,
    pub(super) server: &'a str,
    pub(super) port: u16,
    pub(super) resume: shadowsocks::ShadowsocksUdpFlowResume,
    pub(super) target: &'a Address,
    pub(super) target_port: u16,
    pub(super) payload: &'a [u8],
}
