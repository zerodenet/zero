use tokio::task::JoinSet;
use zero_core::Address;
use zero_engine::EngineError;

use super::model::{VmessUdpRelayFlowStart, VmessUdpStartFlow, VmessUdpUpstreamRequest};
use super::{establish, VmessUdpOutboundManager};
use crate::runtime::udp_flow::managed::{
    ManagedStreamConnection, ManagedStreamConnectionCacheKey, ManagedStreamFlowSender,
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
        self.insert_upstream(
            ManagedStreamConnectionCacheKey::new(
                request.session.target.clone(),
                request.session.port,
            ),
            upstream,
        );
        self.spawn_bridge(
            chain_tasks,
            request.session.target.clone(),
            request.session.port,
            request.session.id,
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
        let Some(upstream) = self.upstreams.get(&key) else {
            return Ok(None);
        };

        proxy.record_session_inbound_rx(upstream.session_id, payload.len() as u64);
        let packet_len = upstream.connection.send(target, port, payload).await? as u64;
        proxy.record_session_outbound_tx(upstream.session_id, packet_len);
        self.spawn_bridge(chain_tasks, target.clone(), port, upstream.session_id);
        Ok(Some(upstream.session_id))
    }

    fn insert_upstream(
        &mut self,
        key: ManagedStreamConnectionCacheKey,
        upstream: ManagedStreamConnection,
    ) {
        self.upstreams.insert(key, upstream);
    }

    async fn get_or_create_upstream(
        &mut self,
        chain_tasks: &mut JoinSet<crate::runtime::udp_flow::packet_path::ChainTask>,
        request: VmessUdpUpstreamRequest<'_>,
    ) -> Result<(), EngineError> {
        let key = ManagedStreamConnectionCacheKey::new(request.target.clone(), request.port);
        if let Some(upstream) = self.upstreams.get(&key) {
            request.proxy.record_session_inbound_rx(
                upstream.session_id,
                request.initial_payload.len() as u64,
            );
            let packet_len = upstream
                .connection
                .send(&request.target, request.port, request.initial_payload)
                .await? as u64;
            request
                .proxy
                .record_session_outbound_tx(upstream.session_id, packet_len);
            self.spawn_bridge(
                chain_tasks,
                request.target,
                request.port,
                upstream.session_id,
            );
            return Ok(());
        }

        let upstream = establish::direct(&request).await?;
        let session_id = upstream.session_id;
        self.upstreams.insert(key, upstream);
        self.spawn_bridge(chain_tasks, request.target, request.port, session_id);
        Ok(())
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
