use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use tracing::debug;
use zero_core::Address;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use super::bridge::recv_loop;
use super::model::{Entry, EntryCandidate};
use crate::protocol_runtime::udp::packet_path_traits::{DatagramCodec, UdpDatagramSource};
use crate::runtime::Proxy;

pub(super) fn resolve_candidate<'a>(
    proxy: &Proxy,
    carrier_leaf: &ResolvedLeafOutbound<'_>,
    datagram_leaf: &ResolvedLeafOutbound<'a>,
) -> Result<EntryCandidate<'a>, EngineError> {
    let (carrier_desc, datagram) = proxy
        .protocols
        .resolve_udp_packet_path_candidate(carrier_leaf, datagram_leaf)?;

    debug!(
        carrier = %carrier_desc.cache_key,
        carrier_server = %carrier_desc.server,
        carrier_port = carrier_desc.port,
        datagram_tag = %datagram.tag,
        datagram_server = %datagram.server,
        datagram_port = datagram.port,
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
    candidate: EntryCandidate<'_>,
) -> Result<Entry, EngineError> {
    let path = proxy
        .protocols
        .build_udp_packet_path_carrier(proxy, carrier_leaf)
        .await?;
    let codec = datagram_codec(&candidate.datagram)?;
    let waiters = Arc::new(Mutex::new(VecDeque::new()));
    tokio::spawn(recv_loop(path.clone(), waiters.clone(), codec.clone()));

    Ok(Entry {
        path,
        waiters,
        codec,
        datagram_server: candidate.datagram.server.to_owned(),
        datagram_port: candidate.datagram.port,
    })
}

fn datagram_codec(
    datagram: &UdpDatagramSource<'_>,
) -> Result<Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>, EngineError> {
    Ok(Arc::new(shadowsocks::ShadowsocksDatagramCodec {
        cipher: datagram.cipher_kind,
        password: datagram.password.as_bytes().to_vec(),
    }))
}
