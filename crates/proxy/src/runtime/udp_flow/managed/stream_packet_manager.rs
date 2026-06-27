use std::future::Future;

use tokio::task::JoinSet;
use zero_core::Address;
use zero_engine::EngineError;

use super::{ManagedStreamConnection, ManagedStreamConnectionCache, ManagedStreamConnectionSend};
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::Proxy;

pub(crate) struct ManagedStreamPacketSender {
    upstreams: ManagedStreamConnectionCache,
}

impl ManagedStreamPacketSender {
    pub(crate) fn new() -> Self {
        Self {
            upstreams: ManagedStreamConnectionCache::new(),
        }
    }

    pub(crate) async fn send_or_insert_target<Fut>(
        &mut self,
        target: &Address,
        port: u16,
        request: ManagedStreamConnectionSend<'_>,
        establish: Fut,
    ) -> Result<(), EngineError>
    where
        Fut: Future<Output = Result<ManagedStreamConnection, EngineError>>,
    {
        self.upstreams
            .send_or_insert_target(target, port, request, establish)
            .await
    }

    pub(crate) fn insert_and_bridge_target(
        &mut self,
        target: Address,
        port: u16,
        chain_tasks: &mut JoinSet<ChainTask>,
        upstream: ManagedStreamConnection,
    ) {
        self.upstreams
            .insert_and_bridge_target(target, port, chain_tasks, upstream);
    }

    pub(crate) async fn send_existing_target(
        &self,
        target: &Address,
        port: u16,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        payload: &[u8],
    ) -> Result<Option<u64>, EngineError> {
        self.upstreams
            .send_existing_target(target, port, chain_tasks, proxy, payload)
            .await
    }
}

#[async_trait::async_trait]
impl super::stream_sender::ManagedStreamFlowSender for ManagedStreamPacketSender {
    async fn send_existing(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<u64>, EngineError> {
        self.send_existing_target(target, port, chain_tasks, proxy, payload)
            .await
    }
}
