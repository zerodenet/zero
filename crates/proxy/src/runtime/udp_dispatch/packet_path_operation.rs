use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use zero_engine::EngineError;

use crate::protocol_registry::UdpAdapterContext;
use crate::runtime::udp_flow::packet_path::{
    PacketPathCarrier, PacketPathCarrierDescriptor, UdpDatagramSource,
};

pub(crate) type PacketPathCarrierFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Arc<dyn PacketPathCarrier>, EngineError>> + Send + 'a>>;

pub(crate) trait PreparedUdpPacketPathOperation: Send {
    fn into_carrier_descriptor(self: Box<Self>) -> Option<PacketPathCarrierDescriptor> {
        None
    }

    fn into_datagram_source(self: Box<Self>) -> Option<UdpDatagramSource> {
        None
    }

    fn build_carrier<'a>(
        self: Box<Self>,
        _ctx: UdpAdapterContext<'a>,
    ) -> PacketPathCarrierFuture<'a>
    where
        Self: 'a,
    {
        Box::pin(async {
            Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "UDP packet-path carrier build is unsupported",
            )))
        })
    }
}
