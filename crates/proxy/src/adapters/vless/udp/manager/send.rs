use tokio::task::JoinSet;
use zero_core::Address;
use zero_engine::EngineError;

use super::model::{
    VlessUdpRelayFinalHopStart, VlessUdpRelayTwoStream, VlessUdpStartFlow, VlessUdpUpstreamRequest,
};
use super::{establish, VlessUdpOutboundManager};
use crate::adapters::vless::mux_pool::VlessMuxOpenRequest;
use crate::runtime::udp_flow::managed::{
    ManagedStreamConnection, ManagedStreamConnectionCacheKey, ManagedStreamFlowSender,
};
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::Proxy;

impl VlessUdpOutboundManager {
    pub(crate) async fn start_flow(
        &mut self,
        chain_tasks: &mut JoinSet<crate::runtime::udp_flow::packet_path::ChainTask>,
        request: VlessUdpStartFlow<'_>,
    ) -> Result<(), EngineError> {
        if request.config.mux_flow_enabled() {
            let max_concurrency = 8u32;
            if let Ok((_mux_sid, up_tx, _down_rx)) = request
                .mux_pool
                .open_udp_stream(VlessMuxOpenRequest {
                    proxy: request.proxy,
                    session: None,
                    server: request.server,
                    port: request.port,
                    id: request.config.uuid(),
                    tls: request.transport.tls,
                    reality: request.transport.reality,
                    max_concurrency,
                })
                .await
            {
                let packet = request.config.encode_initial_flow_packet(
                    &request.session.target,
                    request.session.port,
                    request.payload,
                )?;
                let sent = packet.len();
                let _ = up_tx.send(packet);
                request
                    .proxy
                    .record_session_outbound_tx(request.session.id, sent as u64);
                return Ok(());
            }
        }

        self.get_or_create_upstream(
            chain_tasks,
            VlessUdpUpstreamRequest {
                proxy: request.proxy,
                session: request.session,
                target: request.session.target.clone(),
                port: request.session.port,
                server: request.server,
                server_port: request.port,
                config: request.config,
                initial_payload: request.payload,
                transport: Some(&request.transport),
            },
        )
        .await
    }

    pub(crate) async fn start_relay_two_stream(
        &mut self,
        chain_tasks: &mut JoinSet<crate::runtime::udp_flow::packet_path::ChainTask>,
        request: VlessUdpRelayTwoStream<'_>,
    ) -> Result<(), EngineError> {
        let stream = crate::transport::build_vless_split_http_over_relay(
            request.post_carrier.stream,
            request.get_carrier.stream,
            request.split_http,
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

    pub(crate) async fn start_relay_final_hop(
        &mut self,
        chain_tasks: &mut JoinSet<crate::runtime::udp_flow::packet_path::ChainTask>,
        request: VlessUdpRelayFinalHopStart<'_>,
    ) -> Result<(), EngineError> {
        let stream = crate::transport::build_vless_outbound_transport_over_stream(
            crate::transport::VlessFinalHopTransportRequest {
                carrier: request.carrier,
                options: crate::transport::VlessTransportOptions {
                    tls: request.transport.tls,
                    reality: request.transport.reality,
                    ws: request.transport.ws,
                    grpc: request.transport.grpc,
                    h2: request.transport.h2,
                    http_upgrade: request.transport.http_upgrade,
                    split_http: request.transport.split_http,
                    source_dir: request.transport.source_dir,
                },
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
        request: VlessUdpUpstreamRequest<'_>,
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

        let upstream = establish::direct(
            request.proxy,
            request.session,
            request.server,
            request.server_port,
            request.config,
            request.initial_payload,
            request.transport,
        )
        .await?;
        let session_id = upstream.session_id;
        self.upstreams.insert(key, upstream);
        self.spawn_bridge(chain_tasks, request.target, request.port, session_id);
        Ok(())
    }
}

#[async_trait::async_trait]
impl ManagedStreamFlowSender for VlessUdpOutboundManager {
    async fn send_existing(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<u64>, EngineError> {
        VlessUdpOutboundManager::send_existing(self, chain_tasks, proxy, target, port, payload)
            .await
    }
}
