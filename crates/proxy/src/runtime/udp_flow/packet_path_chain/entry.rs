use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use tracing::debug;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use super::bridge::recv_loop;
use super::model::{Entry, EntryCandidate};
use crate::runtime::Proxy;

pub(super) fn resolve_candidate(
    proxy: &Proxy,
    carrier_leaf: &ResolvedLeafOutbound<'_>,
    datagram_leaf: &ResolvedLeafOutbound<'_>,
) -> Result<EntryCandidate, EngineError> {
    let (carrier_desc, datagram) = proxy
        .protocols
        .resolve_udp_packet_path_candidate(carrier_leaf, datagram_leaf)?;

    let datagram_desc = datagram.descriptor();
    debug!(
        carrier = %carrier_desc.cache_key,
        carrier_server = %carrier_desc.server,
        carrier_port = carrier_desc.port,
        datagram_tag = %datagram_desc.tag,
        datagram_server = %datagram_desc.server,
        datagram_port = datagram_desc.port,
        "ensuring UDP packet-path relay chain"
    );

    Ok(EntryCandidate {
        carrier_desc,
        datagram,
    })
}

pub(super) async fn build_entry(
    proxy: &Proxy,
    carrier_leaf: &ResolvedLeafOutbound<'_>,
    candidate: EntryCandidate,
) -> Result<Entry, EngineError> {
    let path = proxy
        .protocols
        .build_udp_packet_path_carrier(proxy, carrier_leaf)
        .await?;
    let codec = candidate.datagram.codec.clone();
    let datagram_desc = candidate.datagram.descriptor();
    let waiters = Arc::new(Mutex::new(VecDeque::new()));
    tokio::spawn(recv_loop(path.clone(), waiters.clone(), codec.clone()));

    Ok(Entry {
        path,
        waiters,
        codec,
        datagram_endpoint: datagram_desc.endpoint(),
    })
}
