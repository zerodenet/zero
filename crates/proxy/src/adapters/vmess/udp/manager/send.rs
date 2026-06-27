use tokio::task::JoinSet;
use zero_core::Address;
use zero_engine::EngineError;

use super::model::{VmessUdpRelayFlowStart, VmessUdpStartFlow, VmessUdpUpstreamRequest};
use super::{establish, VmessUdpOutboundManager};
use crate::runtime::udp_flow::managed::{
    ManagedStreamConnectionCacheKey, ManagedStreamConnectionSend, ManagedStreamFlowSender,
};
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::Proxy;

impl VmessUdpOutboundManager {
    pub(crate) async fn start_flow(
        &mut self,
        chain_tasks: &mut JoinSet<crate::runtime::udp_flow::packet_path::ChainTask>,
        request: VmessUdpStartFlow<'_>,
    ) -> Result<(), EngineError> {
        self.get_or_create_upstream(
            chain_tasks,
            VmessUdpUpstreamRequest {
                proxy: request.proxy,
                session: request.session,
                target: request.session.target.clone(),
                port: request.session.port,
                server: request.server,
                server_port: request.port,
                config: request.config,
                mux_pool: request.mux_pool,
                initial_payload: request.payload,
                transport: Some(&request.transport),
                mux_concurrency: request.mux_concurrency,
            },
        )
        .await
    }

    pub(crate) async fn start_relay_flow(
        &mut self,
        chain_tasks: &mut JoinSet<crate::runtime::udp_flow::packet_path::ChainTask>,
        request: VmessUdpRelayFlowStart<'_>,
    ) -> Result<(), EngineError> {
        let stream = crate::transport::build_vmess_outbound_transport_over_stream(
            crate::transport::VmessFinalHopTransportRequest {
                carrier: request.carrier,
                options: request.transport,
            },
        )
        .await?;
        let upstream = establish::over_stream(
            request.proxy,
            request.session,
            request.config,
            request.payload,
            stream,
        )
        .await?;
        self.upstreams.insert_and_bridge(
            ManagedStreamConnectionCacheKey::new(
                request.session.target.clone(),
                request.session.port,
            ),
            chain_tasks,
            upstream,
        );
        Ok(())
    }

    pub(crate) async fn send_existing(
        &self,
        chain_tasks: &mut JoinSet<crate::runtime::udp_flow::packet_path::ChainTask>,
        proxy: &Proxy,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<u64>, EngineError> {
        let key = ManagedStreamConnectionCacheKey::new(target.clone(), port);
        self.upstreams
            .send_existing(key, chain_tasks, proxy, target, port, payload)
            .await
    }

    async fn get_or_create_upstream(
        &mut self,
        chain_tasks: &mut JoinSet<crate::runtime::udp_flow::packet_path::ChainTask>,
        request: VmessUdpUpstreamRequest<'_>,
    ) -> Result<(), EngineError> {
        let key = ManagedStreamConnectionCacheKey::new(request.target.clone(), request.port);
        self.upstreams
            .send_or_insert(
                key,
                ManagedStreamConnectionSend {
                    chain_tasks,
                    proxy: request.proxy,
                    target: &request.target,
                    port: request.port,
                    payload: request.initial_payload,
                },
                establish::direct(&request),
            )
            .await
    }
}

#[async_trait::async_trait]
impl ManagedStreamFlowSender for VmessUdpOutboundManager {
    async fn send_existing(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<u64>, EngineError> {
        VmessUdpOutboundManager::send_existing(self, chain_tasks, proxy, target, port, payload)
            .await
    }
}
