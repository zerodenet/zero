use super::super::packet_path_traits::{H2UdpPeer, UdpFlowContext, UdpPacketRef, UdpPeerEndpoint};
use super::super::FlowFailure;
use super::model::{H2Entry, H2Key};
use super::{codec, establish, H2ChainManager, H2SendExisting};

impl H2ChainManager {
    async fn send(
        &mut self,
        ctx: UdpFlowContext<'_>,
        peer: H2UdpPeer<'_>,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let sent = packet_ref.payload.len();
        let key = H2Key::from_peer(&peer);

        if let Some(entry) = self.upstreams.get(&key) {
            let dg = codec::packet(packet_ref.target, packet_ref.port, packet_ref.payload)
                .map_err(|error| FlowFailure {
                    stage: "h2_udp_packet",
                    error,
                    upstream: Some(peer.endpoint.upstream()),
                })?;
            let _ = entry.send_tx.send(dg).await;
            return Ok(sent);
        }

        let send_tx = establish::upstream(ctx.chain_tasks, ctx.session_id, &peer, packet_ref)
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
            },
        );

        Ok(sent)
    }

    pub(crate) async fn send_existing(
        &mut self,
        request: H2SendExisting<'_>,
    ) -> Result<usize, FlowFailure> {
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
                password: request.password,
                client_fingerprint: request.client_fingerprint,
            },
            UdpPacketRef {
                target: request.target,
                port: request.target_port,
                payload: request.payload,
            },
        )
        .await
    }
}
