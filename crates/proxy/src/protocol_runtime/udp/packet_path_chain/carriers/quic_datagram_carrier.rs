use std::sync::Arc;

use async_trait::async_trait;
use zero_core::Address;
use zero_engine::EngineError;
use zero_traits::DatagramCodec;
use zero_transport::udp_packet_path::QuicDatagramPacketPath;

use crate::protocol_runtime::udp::PacketPathCarrier;

pub(crate) async fn build(
    conn: Arc<quinn::Connection>,
    codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
) -> Result<Arc<dyn PacketPathCarrier>, EngineError> {
    let path = QuicDatagramPacketPath::new(conn, codec);
    Ok(Arc::new(PacketPathCarrierAdapter(path)))
}

struct PacketPathCarrierAdapter(QuicDatagramPacketPath);

#[async_trait]
impl PacketPathCarrier for PacketPathCarrierAdapter {
    async fn send_to(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<(), EngineError> {
        self.0.send_to(target, port, payload).await
    }

    async fn recv_from(&self, buf: &mut [u8]) -> Result<usize, EngineError> {
        self.0.recv_from(buf).await
    }
}
