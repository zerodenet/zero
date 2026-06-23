use zero_core::Session;
use zero_engine::EngineError;

use super::model::{MieruKey, MieruRelayExisting, MieruSendExisting};
use super::{bridge, codec, establish, MieruChainManager};
use crate::protocol_runtime::udp::packet_path_traits::{
    MieruUdpPeer, UdpFlowContext, UdpPacketRef, UdpPeerEndpoint,
};
use crate::protocol_runtime::udp::FlowFailure;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

impl MieruChainManager {
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
        let key = if peer.relay_chain {
            MieruKey::Relay { session_id }
        } else {
            MieruKey::Leaf {
                server: peer.endpoint.server.to_owned(),
                port: peer.endpoint.port,
                username: peer.username.to_owned(),
                password: peer.password.to_owned(),
            }
        };

        if let Some(entry) = self.upstreams.get(&key) {
            bridge::spawn_response_bridge(ctx.chain_tasks, entry.recv_tx.clone(), session_id);
            let wrapped = codec::packet(packet_ref.target, packet_ref.port, packet_ref.payload)
                .map_err(|error| FlowFailure {
                    stage: "mieru_udp_packet",
                    error,
                    upstream: Some(peer.endpoint.upstream()),
                })?;
            let _ = entry.send_tx.send(wrapped).await;
            return Ok(sent);
        }

        if peer.relay_chain {
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
        let send_tx = entry.send_tx.clone();
        self.upstreams.insert(key, entry);

        let wrapped = codec::packet(packet_ref.target, packet_ref.port, packet_ref.payload)
            .map_err(|error| FlowFailure {
                stage: "mieru_udp_packet",
                error,
                upstream: Some(peer.endpoint.upstream()),
            })?;
        let _ = send_tx.send(wrapped).await;
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
                endpoint: UdpPeerEndpoint {
                    server: request.server,
                    port: request.port,
                },
                username: request.username,
                password: request.password,
                relay_chain: request.relay_chain,
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
        let entry = establish::packet_stream(stream, peer.username, peer.password)
            .await
            .map_err(|e| FlowFailure {
                stage: "mieru_relay_establish",
                error: e,
                upstream: Some(peer.endpoint.upstream()),
            })?;

        bridge::spawn_response_bridge(ctx.chain_tasks, entry.recv_tx.clone(), session_id);
        let send_tx = entry.send_tx.clone();
        self.upstreams.insert(key, entry);

        let wrapped = codec::packet(packet_ref.target, packet_ref.port, packet_ref.payload)
            .map_err(|error| FlowFailure {
                stage: "mieru_udp_packet",
                error,
                upstream: Some(peer.endpoint.upstream()),
            })?;
        let _ = send_tx.send(wrapped).await;
        Ok(packet_ref.payload.len())
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
                endpoint: UdpPeerEndpoint {
                    server: request.server,
                    port: request.port,
                },
                username: request.username,
                password: request.password,
                relay_chain: true,
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
