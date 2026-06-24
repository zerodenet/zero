//! Datagram-over-packet-path manager for UDP relay chains.
//!
//! Models the relay pattern where the first hop (carrier) provides a raw
//! send/recv channel ([`PacketPathCarrier`]) and the final hop (datagram)
//! encodes its protocol datagrams through that channel ([`DatagramCodec`]).
//!
//! The manager keeps the cache and send/forward entrypoints. Adapter role
//! resolution and entry construction live in `packet_path_chain/entry.rs`.

use std::collections::HashMap;

use super::packet_path_traits::{UdpFlowContext, UdpPacketRef};
use super::FlowFailure;
use crate::runtime::Proxy;
use zero_engine::{EngineError, ResolvedLeafOutbound};

mod bridge;
mod carriers;
mod diagnostics;
mod entry;
mod key;
mod model;
mod snapshot;

use bridge::dispatch_via_entry;
#[cfg(feature = "hysteria2")]
pub(crate) use carriers::build_hysteria2_packet_path;
pub(crate) use carriers::build_shadowsocks_packet_path;
use entry::build_entry;
use key::PathKey;
use model::Entry;

pub(crate) struct PacketPathManager {
    upstreams: HashMap<PathKey, Entry>,
}

impl PacketPathManager {
    pub(crate) fn new() -> Self {
        Self {
            upstreams: HashMap::new(),
        }
    }

    /// Start path: resolve carrier+datagram via the adapter registry, build on
    /// cache miss, encode + send. Takes the resolved leaves directly.
    pub(crate) async fn send(
        &mut self,
        ctx: UdpFlowContext<'_>,
        proxy: &Proxy,
        carrier_leaf: &ResolvedLeafOutbound<'_>,
        datagram_leaf: &ResolvedLeafOutbound<'_>,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let entry = self
            .ensure_entry(proxy, carrier_leaf, datagram_leaf)
            .await
            .map_err(|error| FlowFailure {
                stage: "packet_path_establish",
                error,
                upstream: Some(diagnostics::carrier_upstream(proxy, carrier_leaf)),
            })?;
        dispatch_via_entry(entry, ctx, packet_ref).await
    }

    /// Forward path: the carrier was cached at start time; look it up by the
    /// stored snapshot's cache key. No leaves available, so no re-dial.
    pub(crate) async fn send_with_snapshot(
        &mut self,
        request: SendWithSnapshotRequest<'_>,
    ) -> Result<usize, FlowFailure> {
        let entry = snapshot::lookup_entry(
            &self.upstreams,
            snapshot::SnapshotLookup {
                carrier: request.carrier,
                datagram_tag: request.datagram_tag,
                datagram_server: request.datagram_server,
                datagram_port: request.datagram_port,
                datagram_cache_key: request.datagram_cache_key,
            },
        )?;
        dispatch_via_entry(entry, request.ctx, request.packet_ref).await
    }

    async fn ensure_entry(
        &mut self,
        proxy: &Proxy,
        carrier_leaf: &ResolvedLeafOutbound<'_>,
        datagram_leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<&Entry, EngineError> {
        let candidate = entry::resolve_candidate(proxy, carrier_leaf, datagram_leaf)?;
        let key = candidate.key();

        if !self.upstreams.contains_key(&key) {
            let entry = build_entry(proxy, carrier_leaf, candidate).await?;
            self.upstreams.insert(key.clone(), entry);
        }

        Ok(self
            .upstreams
            .get(&key)
            .expect("packet path entry inserted"))
    }
}

pub(crate) struct SendWithSnapshotRequest<'a> {
    pub ctx: UdpFlowContext<'a>,
    pub carrier: &'a crate::protocol_runtime::udp::UdpPacketPathCarrier,
    pub datagram_tag: &'a str,
    pub datagram_server: &'a str,
    pub datagram_port: u16,
    pub datagram_cache_key: &'a str,
    pub packet_ref: UdpPacketRef<'a>,
}
