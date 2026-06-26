use std::collections::HashMap;
use std::sync::Arc;

use super::packet_path_traits::{UdpFlowContext, UdpPacketRef};
use super::FlowFailure;
use super::{SsUdpPeer, UdpPeerEndpoint};
use crate::runtime::Proxy;

mod bridge;
mod entry;
pub(super) mod model;

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
        resume: shadowsocks::ShadowsocksUdpFlowResume,
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

        let entry = entry::ensure(&mut self.upstreams, peer.leaf_key, resume, target_addr)
            .await
            .map_err(|error| FlowFailure {
                stage: "ss_establish",
                error,
                upstream: Some(peer.endpoint.upstream()),
            })?;

        let packet =
            shadowsocks::udp_flow_packet(packet_ref.target, packet_ref.port, packet_ref.payload);

        let response_rx = entry.waiters.register(packet_ref.target, packet_ref.port);
        if let Err(e) = entry.flow.send_packet(packet).await {
            entry.waiters.remove(packet_ref.target, packet_ref.port);
            return Err(FlowFailure {
                stage: "ss_send",
                error: e,
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
        let leaf_key = request.resume.leaf_cache_key();
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
                leaf_key,
            },
            request.resume,
            UdpPacketRef {
                target: request.target,
                port: request.target_port,
                payload: request.payload,
            },
        )
        .await
    }
}
