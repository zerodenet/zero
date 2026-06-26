use std::sync::Arc;

use async_trait::async_trait;
use zero_core::Address;
use zero_engine::EngineError;

use crate::protocol_runtime::udp::PacketPathCarrier;

/// QUIC-backed packet path carrier for Hysteria2.
pub(super) struct Hysteria2PacketPath {
    conn: Arc<quinn::Connection>,
}

impl Hysteria2PacketPath {
    pub(super) async fn establish(
        server: &str,
        port: u16,
        password: &str,
        client_fingerprint: Option<&str>,
    ) -> Result<Self, EngineError> {
        let connector = crate::transport::Hysteria2Connector::new(server, port, password)
            .with_fingerprint(client_fingerprint);
        let conn = Arc::new(connector.connect_raw().await?);
        Ok(Self { conn })
    }

    fn encode(target: &Address, port: u16, payload: &[u8]) -> Result<Vec<u8>, EngineError> {
        hysteria2::encode_udp_flow_packet(target, port, payload).map_err(EngineError::from)
    }

    fn decode(data: &[u8]) -> Result<Vec<u8>, EngineError> {
        let pkt = hysteria2::decode_udp_flow_packet(data).map_err(EngineError::from)?;
        Ok(pkt.payload)
    }
}

#[async_trait]
impl PacketPathCarrier for Hysteria2PacketPath {
    async fn send_to(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<(), EngineError> {
        let datagram = Self::encode(target, port, payload)?;
        self.conn.send_datagram(datagram.into()).map_err(|e| {
            EngineError::Io(std::io::Error::other(format!(
                "hysteria2 carrier send: {e}"
            )))
        })?;
        Ok(())
    }

    async fn recv_from(&self, buf: &mut [u8]) -> Result<usize, EngineError> {
        let data = self.conn.read_datagram().await.map_err(|e| {
            EngineError::Io(std::io::Error::other(format!(
                "hysteria2 carrier recv: {e}"
            )))
        })?;
        let payload = Self::decode(&data)?;
        let len = payload.len();
        if len > buf.len() {
            return Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "hysteria2 carrier datagram ({len}B) exceeds recv buffer ({}B)",
                    buf.len()
                ),
            )));
        }
        buf[..len].copy_from_slice(&payload);
        Ok(len)
    }
}
