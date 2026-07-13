use std::collections::HashMap;

use super::key::PathKey;
use super::model::Entry;
use crate::runtime::udp_flow::packet_path::PacketPathLookupKey;
use crate::runtime::udp_flow::result::FlowFailure;
use zero_engine::EngineError;

pub(super) struct SnapshotLookup {
    pub(super) lookup_key: PacketPathLookupKey,
}

pub(super) fn lookup_entry(
    upstreams: &HashMap<PathKey, Entry>,
    lookup: SnapshotLookup,
) -> Result<&Entry, FlowFailure> {
    let upstream = lookup.lookup_key.datagram_endpoint();
    let key = PathKey::from_lookup(lookup.lookup_key);
    upstreams.get(&key).ok_or_else(|| FlowFailure {
        stage: "packet_path_carrier_dropped",
        error: EngineError::Io(std::io::Error::other(
            "cached packet-path carrier not found",
        )),
        upstream: Some(upstream),
    })
}
