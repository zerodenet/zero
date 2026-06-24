use std::collections::HashMap;

use super::key::PathKey;
use super::model::Entry;
use crate::protocol_runtime::udp::UdpPacketPathCarrier;
use crate::runtime::udp_dispatch::FlowFailure;
use zero_engine::EngineError;

pub(super) struct SnapshotLookup<'a> {
    pub(super) carrier: &'a UdpPacketPathCarrier,
    pub(super) datagram_tag: &'a str,
    pub(super) datagram_server: &'a str,
    pub(super) datagram_port: u16,
    pub(super) datagram_cache_key: &'a str,
}

pub(super) fn lookup_entry<'a>(
    upstreams: &'a HashMap<PathKey, Entry>,
    lookup: SnapshotLookup<'_>,
) -> Result<&'a Entry, FlowFailure> {
    let key = PathKey::from_snapshot(
        lookup.carrier,
        lookup.datagram_tag,
        lookup.datagram_server,
        lookup.datagram_port,
        lookup.datagram_cache_key,
    );
    upstreams.get(&key).ok_or_else(|| FlowFailure {
        stage: "packet_path_carrier_dropped",
        error: EngineError::Io(std::io::Error::other(
            "cached packet-path carrier not found",
        )),
        upstream: Some((lookup.datagram_server.to_owned(), lookup.datagram_port)),
    })
}
