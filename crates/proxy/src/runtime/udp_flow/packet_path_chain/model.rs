use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use zero_core::Address;

use super::bridge::Waiter;
use super::key::PathKey;
use crate::runtime::udp_flow::packet_path::PacketPathCarrier;
use crate::runtime::udp_flow::packet_path::{
    DatagramCodec, PacketPathCarrierDescriptor, UdpDatagramSource,
};

pub(super) struct Entry {
    pub(super) path: Arc<dyn PacketPathCarrier>,
    pub(super) waiters: Arc<Mutex<VecDeque<Waiter>>>,
    pub(super) codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
    pub(super) datagram_server: String,
    pub(super) datagram_port: u16,
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
