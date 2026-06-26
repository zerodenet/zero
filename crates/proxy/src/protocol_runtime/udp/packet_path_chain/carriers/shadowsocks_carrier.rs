use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::net::UdpSocket;
use zero_core::Address;
use zero_engine::EngineError;
use zero_traits::DatagramCodec;

use crate::protocol_runtime::udp::PacketPathCarrier;
use crate::runtime::Proxy;

pub(crate) async fn build(
    proxy: &Proxy,
    server: &str,
    port: u16,
    codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
) -> Result<Arc<dyn PacketPathCarrier>, EngineError> {
    let path = UdpSocketPacketPath::establish(proxy, server, port, codec).await?;
    Ok(Arc::new(path))
}

pub(super) struct UdpSocketPacketPath {
    socket: UdpSocket,
    endpoint: SocketAddr,
    codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
}

impl UdpSocketPacketPath {
    pub(super) async fn establish(
        proxy: &Proxy,
        server: &str,
        port: u16,
        codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
    ) -> Result<Self, EngineError> {
        let endpoint = proxy
            .protocols
            .direct_connector()
            .resolve_address(
                &Address::Domain(server.to_owned()),
                port,
                proxy.resolver.as_ref(),
                "failed to resolve shadowsocks packet path carrier",
            )
            .await?;
        let bind_addr = match endpoint {
            SocketAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
            SocketAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
        };
        let socket = UdpSocket::bind(bind_addr)
            .await
            .map_err(EngineError::from)?;
        Ok(Self {
            socket,
            endpoint,
            codec,
        })
    }
}

#[async_trait]
impl PacketPathCarrier for UdpSocketPacketPath {
    async fn send_to(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<(), EngineError> {
        let packet = self
            .codec
            .encode(target, port, payload)
            .map_err(EngineError::from)?;
        self.socket
            .send_to(&packet, self.endpoint)
            .await
            .map_err(EngineError::from)?;
        Ok(())
    }

    async fn recv_from(&self, buf: &mut [u8]) -> Result<usize, EngineError> {
        let (read, _) = self
            .socket
            .recv_from(buf)
            .await
            .map_err(EngineError::from)?;
        let (_, _, payload) = self.codec.decode(&buf[..read]).ok_or_else(|| {
            EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "failed to decode UDP packet-path datagram",
            ))
        })?;
        let len = payload.len();
        if len > buf.len() {
            return Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "UDP socket carrier datagram ({len}B) exceeds recv buffer ({}B)",
                    buf.len()
                ),
            )));
        }
        buf[..len].copy_from_slice(&payload);
        Ok(len)
    }
}
