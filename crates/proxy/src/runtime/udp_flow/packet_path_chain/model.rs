use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use zero_core::Address;
use zero_engine::ResolvedLeafOutbound;

use super::bridge::Waiter;
use super::key::PathKey;
use crate::runtime::udp_flow::packet_path::PacketPathCarrier;
use crate::runtime::udp_flow::packet_path::{
    DatagramCodec, PacketPathCarrierDescriptor, UdpDatagramEndpoint, UdpDatagramSource,
    UdpPacketRef,
};

pub(crate) struct PacketPathStartRequest<'a> {
    pub(crate) session_id: u64,
    pub(crate) carrier_leaf: &'a ResolvedLeafOutbound<'a>,
    pub(crate) datagram_leaf: &'a ResolvedLeafOutbound<'a>,
    pub(crate) packet: UdpPacketRef<'a>,
}

pub(super) struct Entry {
    pub(super) path: Arc<dyn PacketPathCarrier>,
    pub(super) waiters: Arc<Mutex<VecDeque<Waiter>>>,
    pub(super) codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
    pub(super) datagram_endpoint: UdpDatagramEndpoint,
}

pub(super) struct EntryCandidate {
    pub(super) carrier_desc: PacketPathCarrierDescriptor,
    pub(super) datagram: UdpDatagramSource,
}

impl EntryCandidate {
    pub(super) fn key(&self) -> PathKey {
        PathKey::from_sources(&self.carrier_desc, self.datagram.descriptor().key_part())
    }
}
