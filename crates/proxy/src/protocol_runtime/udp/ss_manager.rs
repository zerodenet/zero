use std::collections::HashMap;
use std::sync::Arc;

use zero_engine::EngineError;

use super::packet_path_traits::{SsUdpPeer, UdpFlowContext, UdpPacketRef, UdpPeerEndpoint};
use super::FlowFailure;
use crate::runtime::Proxy;

mod bridge;
mod codec;
mod entry;
pub(super) mod model;
mod socket;

use model::{SsKey, SsSendExisting, SsUpstream};

pub(crate) struct SsChainManager {
    upstreams: HashMap<SsKey, Arc<SsUpstream>>,
}

impl SsChainManager {
    pub(crate) fn new() -> Self {
        Self {
            upstreams: HashMap::new(),
        }
    }

    async fn send(
        &mut self,
        ctx: UdpFlowContext<'_>,
        proxy: &Proxy,
        peer: SsUdpPeer<'_>,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let target_addr = proxy
            .protocols
            .direct_connector()
            .resolve_address(
                &peer.endpoint.address(),
                peer.endpoint.port,
                proxy.resolver.as_ref(),
                "failed to resolve shadowsocks udp upstream",
            )
            .await
            .map_err(|error| FlowFailure {
                stage: "ss_resolve_addr",
                error: error.into(),
                upstream: Some(peer.endpoint.upstream()),
            })?;

        let entry = entry::ensure(
            &mut self.upstreams,
            peer.endpoint.server,
            peer.endpoint.port,
            peer.password,
            peer.cipher,
            target_addr,
        );

        let packet = codec::encode_packet(
            packet_ref.target,
            packet_ref.port,
            packet_ref.payload,
            peer.cipher,
            peer.password,
        )
        .map_err(|error| FlowFailure {
            stage: "ss_encode",
            error,
            upstream: Some(peer.endpoint.upstream()),
        })?;

        let response_rx = entry.waiters.register(packet_ref.target, packet_ref.port);
        if let Err(e) = entry.socket.send_to(&packet, target_addr).await {
            entry.waiters.remove(packet_ref.target, packet_ref.port);
            return Err(FlowFailure {
                stage: "ss_send",
                error: EngineError::from(e),
                upstream: Some(peer.endpoint.upstream()),
            });
        }

        bridge::spawn_response_bridge(ctx.chain_tasks, response_rx, ctx.session_id);

        Ok(packet_ref.payload.len())
    }

    pub(crate) async fn send_existing(
        &mut self,
        request: SsSendExisting<'_>,
    ) -> Result<usize, FlowFailure> {
        self.send(
            UdpFlowContext {
                chain_tasks: request.chain_tasks,
                session_id: request.session_id,
            },
            request.proxy,
            SsUdpPeer {
                endpoint: UdpPeerEndpoint {
                    server: request.server,
                    port: request.port,
                },
                password: request.password,
                cipher: request.cipher,
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
