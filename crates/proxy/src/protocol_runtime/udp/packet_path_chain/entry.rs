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
    let carrier_adapter = proxy.protocols.find_outbound_leaf(carrier_leaf)?;
    let datagram_adapter = proxy.protocols.find_outbound_leaf(datagram_leaf)?;
    let carrier_desc = carrier_adapter
        .udp_packet_path_carrier_descriptor(carrier_leaf)
        .ok_or_else(|| {
            EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "outbound does not support UDP packet-path carrier role",
            ))
        })?;
    let datagram = datagram_adapter
        .udp_datagram_source(datagram_leaf)
        .ok_or_else(|| {
            EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "outbound does not support UDP packet-path datagram role",
            ))
        })?;

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
    let carrier_adapter = proxy.protocols.find_outbound_leaf(carrier_leaf)?;
    let path = carrier_adapter
        .build_udp_packet_path(proxy, carrier_leaf)
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
    let cipher_kind = shadowsocks::CipherKind::from_str(datagram.cipher).ok_or_else(|| {
        EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("unknown datagram cipher: {}", datagram.cipher),
        ))
    })?;
    Ok(Arc::new(shadowsocks::ShadowsocksDatagramCodec {
        cipher: cipher_kind,
        password: datagram.password.as_bytes().to_vec(),
    }))
}
