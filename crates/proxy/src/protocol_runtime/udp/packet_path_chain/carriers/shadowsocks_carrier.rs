use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use async_trait::async_trait;
use tokio::net::UdpSocket;
use zero_core::Address;
use zero_engine::EngineError;

use crate::protocol_runtime::udp::PacketPathCarrier;
use crate::runtime::Proxy;

pub(super) struct ShadowsocksPacketPath {
    socket: UdpSocket,
    endpoint: SocketAddr,
    cipher: shadowsocks::CipherKind,
    password: Vec<u8>,
}

impl ShadowsocksPacketPath {
    pub(super) async fn establish(
        proxy: &Proxy,
        server: &str,
        port: u16,
        password: &str,
        cipher: shadowsocks::CipherKind,
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
            cipher,
            password: password.as_bytes().to_vec(),
        })
    }
}

#[async_trait]
impl PacketPathCarrier for ShadowsocksPacketPath {
    async fn send_to(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<(), EngineError> {
        let packet =
            shadowsocks::encode_udp_datagram(target, port, payload, self.cipher, &self.password)
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
        let decoded = shadowsocks::decode_udp_datagram(&buf[..read], self.cipher, &self.password)
            .map_err(EngineError::from)?;
        let len = decoded.payload.len();
        buf[..len].copy_from_slice(&decoded.payload);
        Ok(len)
    }
}
