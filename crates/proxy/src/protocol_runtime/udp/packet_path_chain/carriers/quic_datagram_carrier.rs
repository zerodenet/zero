use std::sync::Arc;

use async_trait::async_trait;
use zero_core::Address;
use zero_engine::EngineError;
use zero_traits::DatagramCodec;

use crate::protocol_runtime::udp::PacketPathCarrier;

pub(crate) async fn build(
    server: &str,
    port: u16,
    password: &str,
    client_fingerprint: Option<&str>,
    codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
) -> Result<Arc<dyn PacketPathCarrier>, EngineError> {
    let path = QuicDatagramPacketPath::establish(server, port, password, client_fingerprint, codec)
        .await?;
    Ok(Arc::new(path))
}

pub(super) struct QuicDatagramPacketPath {
    conn: Arc<quinn::Connection>,
    codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
}

impl QuicDatagramPacketPath {
    pub(super) async fn establish(
        server: &str,
        port: u16,
        password: &str,
        client_fingerprint: Option<&str>,
        codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
    ) -> Result<Self, EngineError> {
        let connector = crate::transport::Hysteria2Connector::new(server, port, password)
            .with_fingerprint(client_fingerprint);
        let conn = Arc::new(connector.connect_raw().await?);
        Ok(Self { conn, codec })
    }
}

#[async_trait]
impl PacketPathCarrier for QuicDatagramPacketPath {
    async fn send_to(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<(), EngineError> {
        let datagram = self
            .codec
            .encode(target, port, payload)
            .map_err(EngineError::from)?;
        self.conn.send_datagram(datagram.into()).map_err(|e| {
            EngineError::Io(std::io::Error::other(format!(
                "QUIC datagram packet-path carrier send: {e}"
            )))
        })?;
        Ok(())
    }

    async fn recv_from(&self, buf: &mut [u8]) -> Result<usize, EngineError> {
        let data = self.conn.read_datagram().await.map_err(|e| {
            EngineError::Io(std::io::Error::other(format!(
                "QUIC datagram packet-path carrier recv: {e}"
            )))
        })?;
        let (_, _, payload) = self.codec.decode(&data).ok_or_else(|| {
            EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "failed to decode QUIC packet-path datagram",
            ))
        })?;
        let len = payload.len();
        if len > buf.len() {
            return Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "QUIC datagram packet-path carrier datagram ({len}B) exceeds recv buffer ({}B)",
                    buf.len()
                ),
            )));
        }
        buf[..len].copy_from_slice(&payload);
        Ok(len)
    }
}
