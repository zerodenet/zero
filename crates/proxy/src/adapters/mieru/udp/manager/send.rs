use zero_engine::EngineError;

use super::model::{MieruRelayExisting, MieruSendExisting};
use super::{establish, MieruChainManager};
use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::managed::{
    spawn_tuple_response_bridge, ManagedExistingSend, ManagedRelaySend, ManagedStreamFlowHandler,
    ManagedUdpFlowResume,
};
use crate::runtime::udp_flow::packet_path::{UdpFlowContext, UdpPacketRef};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

impl MieruChainManager {
    fn supports_managed_existing(&self, resume: &ManagedUdpFlowResume) -> bool {
        resume.as_ref::<mieru::MieruUdpFlowResume>().is_some()
    }

    fn supports_managed_relay_existing(&self, resume: &ManagedUdpFlowResume) -> bool {
        resume.as_ref::<mieru::MieruUdpFlowResume>().is_some()
    }

    async fn send(
        &mut self,
        ctx: UdpFlowContext<'_>,
        proxy: &Proxy,
        endpoint: OutboundEndpoint<'_>,
        resume: &mieru::MieruUdpFlowResume,
        relay: bool,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let sent = packet_ref.payload.len();
        let session_id = ctx.session_id;

        if let Some(entry) = self
            .upstreams
            .get(resume, endpoint.server, endpoint.port, session_id)
        {
            spawn_tuple_response_bridge(
                ctx.chain_tasks,
                entry.subscribe_responses(),
                session_id,
                "mieru upstream closed",
            );
            entry
                .send(packet_ref.target, packet_ref.port, packet_ref.payload)
                .await
                .map_err(|error| FlowFailure {
                    stage: "mieru_send",
                    error: EngineError::Io(std::io::Error::other(format!(
                        "mieru udp send: {error}"
                    ))),
                    upstream: Some(endpoint.upstream()),
                })?;
            return Ok(sent);
        }

        if relay {
            return Err(FlowFailure {
                stage: "mieru_relay_upstream",
                error: EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotConnected,
                    "mieru relay upstream is not established",
                )),
                upstream: Some(endpoint.upstream()),
            });
        }

        let entry = establish::direct(proxy, endpoint, resume)
            .await
            .map_err(|e| FlowFailure {
                stage: "mieru_establish",
                error: e,
                upstream: Some(endpoint.upstream()),
            })?;

        spawn_tuple_response_bridge(
            ctx.chain_tasks,
            entry.subscribe_responses(),
            session_id,
            "mieru upstream closed",
        );
        self.upstreams.insert(
            resume,
            endpoint.server,
            endpoint.port,
            session_id,
            entry.clone(),
        );

        entry
            .send(packet_ref.target, packet_ref.port, packet_ref.payload)
            .await
            .map_err(|error| FlowFailure {
                stage: "mieru_send",
                error: EngineError::Io(std::io::Error::other(format!("mieru udp send: {error}"))),
                upstream: Some(endpoint.upstream()),
            })?;
        Ok(sent)
    }

    async fn send_existing(
        &mut self,
        request: MieruSendExisting<'_>,
    ) -> Result<usize, FlowFailure> {
        self.send(
            UdpFlowContext {
                chain_tasks: request.chain_tasks,
                session_id: request.session_id,
            },
            request.proxy,
            OutboundEndpoint {
                server: request.server,
                port: request.port,
            },
            &request.resume,
            request.resume.flow_requires_relay_upstream(),
            UdpPacketRef {
                target: request.target,
                port: request.target_port,
                payload: request.payload,
            },
        )
        .await
    }

    async fn send_relay(
        &mut self,
        ctx: UdpFlowContext<'_>,
        stream: TcpRelayStream,
        endpoint: OutboundEndpoint<'_>,
        resume: &mieru::MieruUdpFlowResume,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let session_id = ctx.session_id;
        let entry = establish::packet_stream(stream, resume)
            .await
            .map_err(|e| FlowFailure {
                stage: "mieru_relay_establish",
                error: e,
                upstream: Some(endpoint.upstream()),
            })?;

        spawn_tuple_response_bridge(
            ctx.chain_tasks,
            entry.subscribe_responses(),
            session_id,
            "mieru upstream closed",
        );
        self.upstreams.insert(
            resume,
            endpoint.server,
            endpoint.port,
            session_id,
            entry.clone(),
        );

        let sent = packet_ref.payload.len();
        entry
            .send(packet_ref.target, packet_ref.port, packet_ref.payload)
            .await
            .map_err(|error| FlowFailure {
                stage: "mieru_relay_send",
                error: EngineError::Io(std::io::Error::other(format!("mieru udp send: {error}"))),
                upstream: Some(endpoint.upstream()),
            })?;
        Ok(sent)
    }

    async fn send_relay_existing(
        &mut self,
        request: MieruRelayExisting<'_>,
    ) -> Result<usize, FlowFailure> {
        self.send_relay(
            UdpFlowContext {
                chain_tasks: request.chain_tasks,
                session_id: request.session_id,
            },
            request.stream,
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

    async fn send_managed_existing(
        &mut self,
        request: ManagedExistingSend<'_>,
    ) -> Result<usize, FlowFailure> {
        let Some(resume) = request.resume.cloned::<mieru::MieruUdpFlowResume>() else {
            return Err(managed_mismatch(
                "udp_mieru_resume",
                request.server,
                request.port,
                "expected Mieru UDP flow resume",
            ));
        };
        let Some(proxy) = request.proxy else {
            return Err(managed_mismatch(
                "udp_mieru_proxy",
                request.server,
                request.port,
                "expected proxy context for Mieru UDP flow",
            ));
        };
        self.send_existing(MieruSendExisting {
            chain_tasks: request.chain_tasks,
            session_id: request.session_id,
            proxy,
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
        let Some(resume) = request.resume.cloned::<mieru::MieruUdpFlowResume>() else {
            return Err(managed_mismatch(
                "udp_mieru_resume",
                request.server,
                request.port,
                "expected Mieru UDP flow resume",
            ));
        };
        self.send_relay_existing(MieruRelayExisting {
            chain_tasks: request.chain_tasks,
            session_id: request.session_id,
            stream: request.stream,
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
impl ManagedStreamFlowHandler for MieruChainManager {
    fn supports_managed_existing(&self, resume: &ManagedUdpFlowResume) -> bool {
        MieruChainManager::supports_managed_existing(self, resume)
    }

    fn supports_managed_relay_existing(&self, resume: &ManagedUdpFlowResume) -> bool {
        MieruChainManager::supports_managed_relay_existing(self, resume)
    }

    async fn send_managed_existing(
        &mut self,
        request: ManagedExistingSend<'_>,
    ) -> Result<usize, FlowFailure> {
        MieruChainManager::send_managed_existing(self, request).await
    }

    async fn send_managed_relay_existing(
        &mut self,
        request: ManagedRelaySend<'_>,
    ) -> Result<usize, FlowFailure> {
        MieruChainManager::send_managed_relay_existing(self, request).await
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
