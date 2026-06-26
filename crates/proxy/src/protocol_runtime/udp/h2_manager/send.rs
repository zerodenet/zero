use super::super::packet_path_traits::{UdpFlowContext, UdpPacketRef};
use super::super::FlowFailure;
use super::super::{H2UdpPeer, UdpPeerEndpoint};
use super::model::{H2Entry, H2Key, H2SendExisting};
use super::{establish, H2ChainManager};
use zero_engine::EngineError;

impl H2ChainManager {
    async fn send(
        &mut self,
        ctx: UdpFlowContext<'_>,
        peer: H2UdpPeer<'_>,
        resume: hysteria2::Hysteria2UdpFlowResume,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let sent = packet_ref.payload.len();
        let key = H2Key::from_peer(&peer);

        if let Some(entry) = self.upstreams.get(&key) {
            let dg = entry
                .resume
                .encode_packet(packet_ref.target, packet_ref.port, packet_ref.payload)
                .map_err(|error| FlowFailure {
                    stage: "h2_udp_packet",
                    error: EngineError::from(error),
                    upstream: Some(peer.endpoint.upstream()),
                })?;
            let _ = entry.send_tx.send(dg).await;
            return Ok(sent);
        }

        let send_tx = establish::upstream(
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
                send_tx: send_tx.clone(),
                resume,
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
                password: request.resume.password(),
                client_fingerprint: request.resume.client_fingerprint(),
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
