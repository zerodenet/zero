//! Datagram-over-packet-path manager for UDP relay chains.
//!
//! Models the relay pattern where the first hop (carrier) provides a raw
//! send/recv channel ([`PacketPathCarrier`]) and the final hop (datagram)
//! encodes its protocol datagrams through that channel ([`DatagramCodec`]).
//!
//! The manager keeps the cache and send/forward entrypoints. Adapter role
//! resolution and entry construction live in `packet_path_chain/entry.rs`.

use std::collections::HashMap;

use crate::runtime::udp_flow::packet_path::{PacketPathLookupKey, UdpFlowContext, UdpPacketRef};
use crate::runtime::udp_flow::result::FlowFailure;
use crate::runtime::Proxy;
use zero_engine::EngineError;

mod bridge;
pub(crate) mod carriers;
mod entry;
mod key;
mod model;
mod snapshot;

use bridge::dispatch_via_entry;
use entry::build_entry;
use key::PathKey;
use model::{Entry, EntryCandidate};
pub(crate) use model::{PacketPathCarrierRequest, PacketPathStartRequest};

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
        request: PacketPathStartRequest<'_>,
    ) -> Result<usize, FlowFailure> {
        let PacketPathStartRequest {
            carrier,
            datagram,
            packet,
            ..
        } = request;
        let upstream = carrier.upstream();
        let entry = self
            .ensure_entry(proxy, carrier, datagram)
            .await
            .map_err(|error| FlowFailure {
                stage: "packet_path_establish",
                error,
                upstream: Some(upstream),
            })?;
        dispatch_via_entry(entry, ctx, packet).await
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
                lookup_key: request.lookup_key,
            },
        )?;
        dispatch_via_entry(entry, request.ctx, request.packet_ref).await
    }

    async fn ensure_entry(
        &mut self,
        proxy: &Proxy,
        carrier: PacketPathCarrierRequest<'_>,
        datagram: crate::runtime::udp_flow::packet_path::UdpDatagramSource,
    ) -> Result<&Entry, EngineError> {
        let candidate = EntryCandidate {
            carrier_desc: carrier.descriptor,
            datagram,
        };
        let key = candidate.key();

        if !self.upstreams.contains_key(&key) {
            let entry = build_entry(proxy, carrier.build_operation, candidate).await?;
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
    pub lookup_key: PacketPathLookupKey,
    pub packet_ref: UdpPacketRef<'a>,
}
