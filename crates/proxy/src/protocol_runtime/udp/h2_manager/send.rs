use super::super::state::managed::model::ManagedExistingSend;
use super::super::FlowFailure;
use super::super::ProtocolUdpFlowResume;
use super::model::{H2Entry, H2Key, H2SendExisting, H2UdpPeer};
use super::{establish, H2ChainManager};
use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_flow::packet_path::{UdpFlowContext, UdpPacketRef};
use zero_core::UdpFlowPacket;

impl H2ChainManager {
    pub(in crate::protocol_runtime::udp) fn supports_managed_existing(
        &self,
        resume: &ProtocolUdpFlowResume,
    ) -> bool {
        resume.as_hysteria2().is_some()
    }

    async fn send(
        &mut self,
        ctx: UdpFlowContext<'_>,
        peer: H2UdpPeer<'_>,
        resume: hysteria2::Hysteria2UdpFlowResume,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let sent = packet_ref.payload.len();
        let key = H2Key::from_flow_key(peer.flow_key.clone());

        if let Some(entry) = self.upstreams.get(&key) {
            let packet =
                UdpFlowPacket::from_parts(packet_ref.target, packet_ref.port, packet_ref.payload);
            return entry
                .send_tx
                .send(packet)
                .await
                .map(|_| sent)
                .map_err(|error| FlowFailure {
                    stage: "h2_send",
                    error: zero_engine::EngineError::Io(std::io::Error::other(format!("{error}"))),
                    upstream: Some(peer.endpoint.upstream()),
                });
        }

        let sender = establish::upstream(
            ctx.chain_tasks,
            ctx.session_id,
            &peer,
            resume.clone(),
            packet_ref,
        )
        .await
        .map_err(|e| FlowFailure {
            stage: "h2_establish",
            error: e,
            upstream: Some(peer.endpoint.upstream()),
        })?;

        self.upstreams.insert(
            key,
            H2Entry {
                send_tx: sender.clone(),
            },
        );

        Ok(sent)
    }

    pub(crate) async fn send_existing(
        &mut self,
        request: H2SendExisting<'_>,
    ) -> Result<usize, FlowFailure> {
        let resume = request.resume.clone();
        self.send(
            UdpFlowContext {
                chain_tasks: request.chain_tasks,
                session_id: request.session_id,
            },
            H2UdpPeer {
                endpoint: OutboundEndpoint {
                    server: request.server,
                    port: request.port,
                },
                flow_key: request.resume.flow_key(request.server, request.port),
            },
            resume,
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
        let Some(resume) = request.resume.into_hysteria2() else {
            return Err(managed_mismatch(
                "udp_hysteria2_resume",
                request.server,
                request.port,
                "expected Hysteria2 UDP flow resume",
            ));
        };
        self.send_existing(H2SendExisting {
            chain_tasks: request.chain_tasks,
            session_id: request.session_id,
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
        error: zero_engine::EngineError::Io(std::io::Error::other(message)),
        upstream: Some((server.to_string(), port)),
    }
}
