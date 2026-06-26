use zero_core::Session;
use zero_engine::EngineError;

use super::model::{TrojanKey, TrojanRelayExisting, TrojanRelaySend, TrojanSendExisting};
use super::{bridge, establish, TrojanChainManager};
use crate::protocol_runtime::udp::packet_path_traits::{UdpFlowContext, UdpPacketRef};
use crate::protocol_runtime::udp::FlowFailure;
use crate::protocol_runtime::udp::{TrojanUdpPeer, UdpPeerEndpoint};
use crate::runtime::Proxy;

impl TrojanChainManager {
    async fn send(
        &mut self,
        ctx: UdpFlowContext<'_>,
        proxy: &Proxy,
        session: &Session,
        peer: TrojanUdpPeer<'_>,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let sent = packet_ref.payload.len();
        let session_id = ctx.session_id;
        let peer_config = peer.resume.peer_config();
        let key = if peer.relay_chain {
            TrojanKey::Relay { session_id }
        } else {
            TrojanKey::Leaf(peer_config.leaf_cache_key(peer.endpoint.server, peer.endpoint.port))
        };

        if let Some(entry) = self.upstreams.get(&key) {
            bridge::spawn_response_bridge(ctx.chain_tasks, entry.recv_tx.clone(), session_id);
            let _ = entry
                .send_tx
                .send(establish::packet(
                    packet_ref.target,
                    packet_ref.port,
                    packet_ref.payload,
                ))
                .await;
            return Ok(sent);
        }

        if peer.relay_chain {
            return Err(FlowFailure {
                stage: "trojan_relay_upstream",
                error: EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotConnected,
                    "trojan relay upstream is not established",
                )),
                upstream: Some(peer.endpoint.upstream()),
            });
        }

        let entry = establish::direct(proxy, session, &peer, packet_ref.target, packet_ref.port)
            .await
            .map_err(|e| FlowFailure {
                stage: "trojan_establish",
                error: e,
                upstream: Some(peer.endpoint.upstream()),
            })?;

        bridge::spawn_response_bridge(ctx.chain_tasks, entry.recv_tx.clone(), session_id);
        let send_tx = entry.send_tx.clone();
        self.upstreams.insert(key, entry);

        let _ = send_tx
            .send(establish::packet(
                packet_ref.target,
                packet_ref.port,
                packet_ref.payload,
            ))
            .await;

        Ok(sent)
    }

    pub(crate) async fn send_existing(
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
            TrojanUdpPeer {
                endpoint: UdpPeerEndpoint {
                    server: request.server,
                    port: request.port,
                },
                resume: &request.resume,
                relay_chain: request.resume.peer_config().relay_chain(),
            },
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
        let peer = request.peer;
        let session_id = ctx.session_id;
        let key = TrojanKey::Relay { session_id };
        let entry = establish::over_relay_stream(
            request.stream,
            request.tls_server_name,
            request.proxy,
            request.session,
            &peer,
            packet_ref.target,
            packet_ref.port,
        )
        .await
        .map_err(|e| FlowFailure {
            stage: "trojan_relay_establish",
            error: e,
            upstream: Some(peer.endpoint.upstream()),
        })?;

        bridge::spawn_response_bridge(ctx.chain_tasks, entry.recv_tx.clone(), session_id);
        let send_tx = entry.send_tx.clone();
        self.upstreams.insert(key, entry);
        let _ = send_tx
            .send(establish::packet(
                packet_ref.target,
                packet_ref.port,
                packet_ref.payload,
            ))
            .await;

        Ok(packet_ref.payload.len())
    }

    pub(crate) async fn send_relay_existing(
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
            peer: TrojanUdpPeer {
                endpoint: UdpPeerEndpoint {
                    server: request.server,
                    port: request.port,
                },
                resume: &request.resume,
                relay_chain: true,
            },
            packet: UdpPacketRef {
                target: request.target,
                port: request.target_port,
                payload: request.payload,
            },
        })
        .await
    }
}
