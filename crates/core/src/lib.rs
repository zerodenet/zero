#![no_std]
#![allow(async_fn_in_trait)]

extern crate alloc;

pub mod address;
pub mod error;

pub mod session;
pub mod udp;

pub use address::{Address, AddressFamily};
pub use error::Error;

pub use session::{Network, ProtocolType, Session, SessionAuth};
pub use udp::{
    DatagramUdpResponder, InboundMuxUdpReadFailure, InboundMuxUdpReadFailureAction,
    InboundMuxUdpRelay, InboundStreamUdpRelay, InboundUdpDispatch, MuxUdpDecodeFailure,
    MuxUdpResponder, StreamUdpResponder, UdpFlowPacket,
};
