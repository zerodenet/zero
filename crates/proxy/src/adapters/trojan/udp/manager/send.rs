use zero_core::Session;
use zero_engine::EngineError;

use super::model::{TrojanRelayExisting, TrojanRelaySend, TrojanSendExisting};
use super::{establish, TrojanChainManager};
use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::managed::{
    ManagedExistingSend, ManagedRelaySend, ManagedStreamFlowHandler, ManagedUdpFlowResume,
};
use crate::runtime::udp_flow::packet_path::{UdpFlowContext, UdpPacketRef};
use crate::runtime::Proxy;

impl TrojanChainManager {
    fn supports_managed_existing(&self, resume: &ManagedUdpFlowResume) -> bool {
        resume.as_ref::<trojan::TrojanUdpFlowResume>().is_some()
    }

    fn supports_managed_relay_existing(&self, resume: &ManagedUdpFlowResume) -> bool {
        resume.as_ref::<trojan::TrojanUdpFlowResume>().is_some()
    }

    async fn send(
        &mut self,
        ctx: UdpFlowContext<'_>,
        proxy: &Proxy,
        session: &Session,
        endpoint: OutboundEndpoint<'_>,
        resume: &trojan::TrojanUdpFlowResume,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let sent = packet_ref.payload.len();
        let session_id = ctx.session_id;

        if let Some(entry) = self
            .upstreams
            .get(resume, endpoint.server, endpoint.port, session_id)
        {
            entry.spawn_response_bridge(ctx.chain_tasks, session_id);
            let _ = entry
                .send(packet_ref.target, packet_ref.port, packet_ref.payload)
                .await;
            return Ok(sent);
        }

        if resume.flow_requires_relay_upstream() {
            return Err(FlowFailure {
                stage: "trojan_relay_upstream",
                error: EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotConnected,
                    "trojan relay upstream is not established",
                )),
                upstream: Some(endpoint.upstream()),
            });
        }

        let entry = establish::direct(proxy, session, endpoint, resume)
            .await
            .map_err(|e| FlowFailure {
                stage: "trojan_establish",
                error: e,
                upstream: Some(endpoint.upstream()),
            })?;

        entry.spawn_response_bridge(ctx.chain_tasks, session_id);
        self.upstreams.insert(
            resume,
            endpoint.server,
            endpoint.port,
            session_id,
            entry.clone(),
        );

        let _ = entry
            .send(packet_ref.target, packet_ref.port, packet_ref.payload)
            .await;

        Ok(sent)
    }

    async fn send_existing(
        &mut self,
        request: TrojanSendExisting<'_>,
    ) -> Result<usize, FlowFailure> {
        self.send(
            UdpFlowContext {
                chain_tasks: request.chain_tasks,
                session_id: request.session_id,
            },
            request.proxy,
            request.session,
            OutboundEndpoint {
                server: request.server,
                port: request.port,
            },
            &request.resume,
            UdpPacketRef {
                target: request.target,
                port: request.target_port,
                payload: request.payload,
            },
        )
        .await
    }

    async fn send_relay(&mut self, request: TrojanRelaySend<'_>) -> Result<usize, FlowFailure> {
        let ctx = request.ctx;
        let packet_ref = request.packet;
        let session_id = ctx.session_id;
        let entry = establish::over_relay_stream(
            request.stream,
            request.tls_server_name,
            request.proxy,
            request.session,
            OutboundEndpoint {
                server: request.server,
                port: request.port,
            },
            request.resume,
        )
        .await
        .map_err(|e| FlowFailure {
            stage: "trojan_relay_establish",
            error: e,
            upstream: Some((request.server.to_owned(), request.port)),
        })?;

        entry.spawn_response_bridge(ctx.chain_tasks, session_id);
        self.upstreams.insert(
            request.resume,
            request.server,
            request.port,
            session_id,
            entry.clone(),
        );
        let _ = entry
            .send(packet_ref.target, packet_ref.port, packet_ref.payload)
            .await;

        Ok(packet_ref.payload.len())
    }

    async fn send_relay_existing(
        &mut self,
        request: TrojanRelayExisting<'_>,
    ) -> Result<usize, FlowFailure> {
        self.send_relay(TrojanRelaySend {
            ctx: UdpFlowContext {
                chain_tasks: request.chain_tasks,
                session_id: request.session_id,
            },
            stream: request.stream,
            tls_server_name: request.tls_server_name,
            proxy: request.proxy,
            session: request.session,
            server: request.server,
            port: request.port,
            resume: &request.resume,
            packet: UdpPacketRef {
                target: request.target,
                port: request.target_port,
                payload: request.payload,
            },
        })
        .await
    }

    async fn send_managed_existing(
        &mut self,
        request: ManagedExistingSend<'_>,
    ) -> Result<usize, FlowFailure> {
        let Some(resume) = request.resume.cloned::<trojan::TrojanUdpFlowResume>() else {
            return Err(managed_mismatch(
                "udp_trojan_resume",
                request.server,
                request.port,
                "expected Trojan UDP flow resume",
            ));
        };
        let Some(proxy) = request.proxy else {
            return Err(managed_mismatch(
                "udp_trojan_proxy",
                request.server,
                request.port,
                "expected proxy context for Trojan UDP flow",
            ));
        };
        self.send_existing(TrojanSendExisting {
            chain_tasks: request.chain_tasks,
            session_id: request.session_id,
            proxy,
            session: request.session,
            server: request.server,
            port: request.port,
            resume,
            target: request.target,
            target_port: request.target_port,
            payload: request.payload,
        })
        .await
    }

    async fn send_managed_relay_existing(
        &mut self,
        request: ManagedRelaySend<'_>,
    ) -> Result<usize, FlowFailure> {
        let Some(resume) = request.resume.cloned::<trojan::TrojanUdpFlowResume>() else {
            return Err(managed_mismatch(
                "udp_trojan_resume",
                request.server,
                request.port,
                "expected Trojan UDP flow resume",
            ));
        };
        let Some(proxy) = request.proxy else {
            return Err(managed_mismatch(
                "udp_trojan_resume",
                request.server,
                request.port,
                "expected Trojan UDP relay proxy context",
            ));
        };
        self.send_relay_existing(TrojanRelayExisting {
            chain_tasks: request.chain_tasks,
            session_id: request.session_id,
            stream: request.stream,
            tls_server_name: request.tls_server_name,
            proxy,
            session: request.session,
            server: request.server,
            port: request.port,
            resume,
            target: request.target,
            target_port: request.target_port,
            payload: request.payload,
        })
        .await
    }
}

#[async_trait::async_trait]
impl ManagedStreamFlowHandler for TrojanChainManager {
    fn supports_managed_existing(&self, resume: &ManagedUdpFlowResume) -> bool {
        TrojanChainManager::supports_managed_existing(self, resume)
    }

    fn supports_managed_relay_existing(&self, resume: &ManagedUdpFlowResume) -> bool {
        TrojanChainManager::supports_managed_relay_existing(self, resume)
    }

    async fn send_managed_existing(
        &mut self,
        request: ManagedExistingSend<'_>,
    ) -> Result<usize, FlowFailure> {
        TrojanChainManager::send_managed_existing(self, request).await
    }

    async fn send_managed_relay_existing(
        &mut self,
        request: ManagedRelaySend<'_>,
    ) -> Result<usize, FlowFailure> {
        TrojanChainManager::send_managed_relay_existing(self, request).await
    }
}

fn managed_mismatch(
    stage: &'static str,
    server: &str,
    port: u16,
    message: &'static str,
) -> FlowFailure {
    FlowFailure {
        stage,
        error: EngineError::Io(std::io::Error::other(message)),
        upstream: Some((server.to_string(), port)),
    }
}
