use super::super::FlowFailure;
use super::super::{H2UdpPeer, UdpPeerEndpoint};
use super::model::{H2Entry, H2Key, H2SendExisting};
use super::{establish, H2ChainManager};
use crate::runtime::udp_flow::packet_path::{UdpFlowContext, UdpPacketRef};
use zero_core::UdpFlowPacket;

impl H2ChainManager {
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
                endpoint: UdpPeerEndpoint {
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
}
