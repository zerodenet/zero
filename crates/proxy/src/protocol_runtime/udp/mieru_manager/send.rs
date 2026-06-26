use zero_core::Session;
use zero_engine::EngineError;

use super::model::{MieruKey, MieruRelayExisting, MieruSendExisting, MieruUdpPeer};
use super::{bridge, establish, MieruChainManager};
use crate::protocol_runtime::udp::state::managed::model::{ManagedExistingSend, ManagedRelaySend};
use crate::protocol_runtime::udp::{FlowFailure, ProtocolUdpFlowResume};
use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_flow::packet_path::{UdpFlowContext, UdpPacketRef};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

impl MieruChainManager {
    pub(in crate::protocol_runtime::udp) fn supports_managed_existing(
        &self,
        resume: &ProtocolUdpFlowResume,
    ) -> bool {
        matches!(resume, ProtocolUdpFlowResume::Mieru(_))
    }

    pub(in crate::protocol_runtime::udp) fn supports_managed_relay_existing(
        &self,
        resume: &ProtocolUdpFlowResume,
    ) -> bool {
        matches!(resume, ProtocolUdpFlowResume::Mieru(_))
    }

    async fn send(
        &mut self,
        ctx: UdpFlowContext<'_>,
        proxy: &Proxy,
        _session: &Session,
        peer: MieruUdpPeer<'_>,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let sent = packet_ref.payload.len();
        let session_id = ctx.session_id;
        let key = MieruKey::from_flow_key(peer.flow_key.clone(), session_id);

        if let Some(entry) = self.upstreams.get(&key) {
            bridge::spawn_response_bridge(ctx.chain_tasks, entry.recv_tx.clone(), session_id);
            entry
                .sender
                .send(packet_ref.target, packet_ref.port, packet_ref.payload)
                .await
                .map_err(|error| FlowFailure {
                    stage: "mieru_send",
                    error: EngineError::Io(std::io::Error::other(format!(
                        "mieru udp send: {error}"
                    ))),
                    upstream: Some(peer.endpoint.upstream()),
                })?;
            return Ok(sent);
        }

        if peer.flow_key.is_relay() {
            return Err(FlowFailure {
                stage: "mieru_relay_upstream",
                error: EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotConnected,
                    "mieru relay upstream is not established",
                )),
                upstream: Some(peer.endpoint.upstream()),
            });
        }

        let entry = establish::direct(proxy, &peer)
            .await
            .map_err(|e| FlowFailure {
                stage: "mieru_establish",
                error: e,
                upstream: Some(peer.endpoint.upstream()),
            })?;

        bridge::spawn_response_bridge(ctx.chain_tasks, entry.recv_tx.clone(), session_id);
        let sender = entry.sender.clone();
        self.upstreams.insert(key, entry);

        sender
            .send(packet_ref.target, packet_ref.port, packet_ref.payload)
            .await
            .map_err(|error| FlowFailure {
                stage: "mieru_send",
                error: EngineError::Io(std::io::Error::other(format!("mieru udp send: {error}"))),
                upstream: Some(peer.endpoint.upstream()),
            })?;
        Ok(sent)
    }

    pub(crate) async fn send_existing(
        &mut self,
        request: MieruSendExisting<'_>,
    ) -> Result<usize, FlowFailure> {
        self.send(
            UdpFlowContext {
                chain_tasks: request.chain_tasks,
                session_id: request.session_id,
            },
            request.proxy,
            request.session,
            MieruUdpPeer {
                endpoint: OutboundEndpoint {
                    server: request.server,
                    port: request.port,
                },
                resume: &request.resume,
                flow_key: request.resume.flow_key(request.server, request.port),
            },
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
        peer: MieruUdpPeer<'_>,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let session_id = ctx.session_id;
        let key = MieruKey::Relay { session_id };
        let entry = establish::packet_stream(stream, peer.resume)
            .await
            .map_err(|e| FlowFailure {
                stage: "mieru_relay_establish",
                error: e,
                upstream: Some(peer.endpoint.upstream()),
            })?;

        bridge::spawn_response_bridge(ctx.chain_tasks, entry.recv_tx.clone(), session_id);
        let sender = entry.sender.clone();
        self.upstreams.insert(key, entry);

        let sent = packet_ref.payload.len();
        sender
            .send(packet_ref.target, packet_ref.port, packet_ref.payload)
            .await
            .map_err(|error| FlowFailure {
                stage: "mieru_relay_send",
                error: EngineError::Io(std::io::Error::other(format!("mieru udp send: {error}"))),
                upstream: Some(peer.endpoint.upstream()),
            })?;
        Ok(sent)
    }

    pub(crate) async fn send_relay_existing(
        &mut self,
        request: MieruRelayExisting<'_>,
    ) -> Result<usize, FlowFailure> {
        self.send_relay(
            UdpFlowContext {
                chain_tasks: request.chain_tasks,
                session_id: request.session_id,
            },
            request.stream,
            MieruUdpPeer {
                endpoint: OutboundEndpoint {
                    server: request.server,
                    port: request.port,
                },
                resume: &request.resume,
                flow_key: mieru::MieruUdpFlowKey::Relay,
            },
            UdpPacketRef {
                target: request.target,
                port: request.target_port,
                payload: request.payload,
            },
        )
        .await
    }

    pub(in crate::protocol_runtime::udp) async fn send_managed_existing(
        &mut self,
        request: ManagedExistingSend<'_>,
    ) -> Result<usize, FlowFailure> {
        let ProtocolUdpFlowResume::Mieru(resume) = request.resume else {
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

    pub(in crate::protocol_runtime::udp) async fn send_managed_relay_existing(
        &mut self,
        request: ManagedRelaySend<'_>,
    ) -> Result<usize, FlowFailure> {
        let ProtocolUdpFlowResume::Mieru(resume) = request.resume else {
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
