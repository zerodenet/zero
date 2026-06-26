use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use zero_core::Address;

use super::bridge::Waiter;
use super::key::PathKey;
use crate::protocol_runtime::udp::packet_path_traits::{
    DatagramCodec, PacketPathCarrierDescriptor, UdpDatagramSource,
};
use crate::protocol_runtime::udp::PacketPathCarrier;

pub(super) struct Entry {
    pub(super) path: Arc<dyn PacketPathCarrier>,
    pub(super) waiters: Arc<Mutex<VecDeque<Waiter>>>,
    pub(super) codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
    pub(super) datagram_server: String,
    pub(super) datagram_port: u16,
}

pub(super) struct EntryCandidate<'a> {
    pub(super) carrier_desc: PacketPathCarrierDescriptor,
    pub(super) datagram: UdpDatagramSource<'a>,
}

impl EntryCandidate<'_> {
    pub(super) fn key(&self) -> PathKey {
        PathKey::from_sources(&self.carrier_desc, self.datagram.descriptor().key_part())
    }
}
