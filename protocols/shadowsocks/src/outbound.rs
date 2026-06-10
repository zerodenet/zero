// Shadowsocks outbound protocol.

use zero_core::ProtocolType;
#[cfg(feature = "crypto")]
use zero_core::{Address, Error, Session};
#[cfg(feature = "crypto")]
use zero_traits::{AsyncSocket, DatagramCodec, TcpSessionProtocol, UdpDatagramFraming};

/// Shadowsocks outbound handler.
#[derive(Debug, Default, Clone, Copy)]
pub struct ShadowsocksOutbound;

#[cfg(feature = "crypto")]
pub struct ShadowsocksOutboundSession {
    pub session_key: Vec<u8>,
    pub next_upload_nonce: u64,
    pub cipher: super::shared::CipherKind,
}

impl ShadowsocksOutbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Shadowsocks
    }

    /// Write the initial encrypted stream payload containing target address.
    #[cfg(feature = "crypto")]
    pub async fn send_request<S: AsyncSocket>(
        &self,
        stream: &mut S,
        session: &Session,
        cipher: super::shared::CipherKind,
        password: &[u8],
    ) -> Result<ShadowsocksOutboundSession, Error> {
        use super::shared::{build_target_data, derive_session_key, encrypt_tcp_chunk};

        let salt_len = cipher.salt_len();

        // Generate random salt
        let mut salt = vec![0u8; salt_len];
        use ring::rand::SecureRandom;
        ring::rand::SystemRandom::new()
            .fill(&mut salt)
            .map_err(|_| Error::Protocol("ss: random failed"))?;

        let key = derive_session_key(cipher, password, &salt)?;

        // Build target data
        let target_data = build_target_data(&session.target, session.port, &[])?;

        let mut nonce = 0;
        let encrypted = encrypt_tcp_chunk(cipher, &key, &mut nonce, &target_data)?;

        let mut request = Vec::with_capacity(salt.len() + encrypted.len());
        request.extend_from_slice(&salt);
        request.extend_from_slice(&encrypted);
        stream
            .write_all(&request)
            .await
            .map_err(|_| Error::Io("ss: failed to write request"))?;

        Ok(ShadowsocksOutboundSession {
            session_key: key,
            next_upload_nonce: nonce,
            cipher,
        })
    }
}

/// Target parameters for Shadowsocks TCP session.
#[cfg(feature = "crypto")]
#[derive(Debug, Clone, Copy)]
pub struct ShadowsocksTcpTarget<'a> {
    pub session: &'a Session,
    pub cipher: super::shared::CipherKind,
    pub password: &'a [u8],
}

#[cfg(feature = "crypto")]
impl<'a> TcpSessionProtocol<ShadowsocksTcpTarget<'a>> for ShadowsocksOutbound {
    type Error = Error;
    type Session = ShadowsocksOutboundSession;

    async fn establish_tcp_session<S>(
        &self,
        stream: &mut S,
        target: &ShadowsocksTcpTarget<'a>,
    ) -> Result<Self::Session, Self::Error>
    where
        S: AsyncSocket,
    {
        self.send_request(stream, target.session, target.cipher, target.password)
            .await
    }
}

/// One plaintext UDP payload to encode into a Shadowsocks UDP datagram.
#[cfg(feature = "crypto")]
#[derive(Debug, Clone, Copy)]
pub struct ShadowsocksUdpPacketTarget<'a> {
    pub target: &'a Address,
    pub port: u16,
    pub payload: &'a [u8],
    pub cipher: super::shared::CipherKind,
    pub password: &'a [u8],
}

/// Decryption context for a Shadowsocks UDP datagram received from upstream.
#[cfg(feature = "crypto")]
#[derive(Debug, Clone, Copy)]
pub struct ShadowsocksUdpDecodeContext<'a> {
    pub cipher: super::shared::CipherKind,
    pub password: &'a [u8],
}

/// One decoded Shadowsocks UDP datagram.
#[cfg(feature = "crypto")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowsocksUdpPacket {
    pub target: Address,
    pub port: u16,
    pub payload: Vec<u8>,
}

#[cfg(feature = "crypto")]
impl<'a> UdpDatagramFraming<ShadowsocksUdpPacketTarget<'a>, ShadowsocksUdpDecodeContext<'a>>
    for ShadowsocksOutbound
{
    type Error = Error;
    type Decoded = ShadowsocksUdpPacket;

    fn encode_udp_datagram(
        &self,
        packet: &ShadowsocksUdpPacketTarget<'a>,
    ) -> Result<Vec<u8>, Self::Error> {
        use super::shared::{aead_encrypt_udp, build_target_data, derive_udp_packet_key};

        if packet.cipher.is_blake3() {
            return super::shared::encode_udp_datagram_2022(
                packet.cipher,
                packet.password,
                packet.target,
                packet.port,
                packet.payload,
            );
        }

        let target_data = build_target_data(packet.target, packet.port, packet.payload)?;
        let mut salt = vec![0u8; packet.cipher.udp_salt_len()];
        use ring::rand::SecureRandom;
        ring::rand::SystemRandom::new()
            .fill(&mut salt)
            .map_err(|_| Error::Protocol("ss: random failed"))?;

        let key = derive_udp_packet_key(packet.cipher, packet.password, &salt)?;

        let encrypted = aead_encrypt_udp(packet.cipher, &key, &[0u8; 12], &target_data)?;
        let mut datagram = Vec::with_capacity(salt.len() + encrypted.len());
        datagram.extend_from_slice(&salt);
        datagram.extend_from_slice(&encrypted);
        Ok(datagram)
    }

    fn decode_udp_datagram(
        &self,
        context: &ShadowsocksUdpDecodeContext<'a>,
        datagram: &[u8],
    ) -> Result<Self::Decoded, Self::Error> {
        use super::shared::{aead_decrypt_udp, derive_udp_packet_key, parse_target_data};

        if context.cipher.is_blake3() {
            let (target, port, payload) = super::shared::decode_udp_datagram_2022(
                context.cipher,
                context.password,
                datagram,
            )?;
            return Ok(ShadowsocksUdpPacket {
                target,
                port,
                payload,
            });
        }

        let salt_len = context.cipher.udp_salt_len();
        if datagram.len() < salt_len + context.cipher.tag_len() {
            return Err(Error::Protocol("ss: udp datagram too short"));
        }

        let key = derive_udp_packet_key(context.cipher, context.password, &datagram[..salt_len])?;

        let plain = aead_decrypt_udp(context.cipher, &key, &[0u8; 12], &datagram[salt_len..])?;
        let (target, port, payload_offset) = parse_target_data(&plain)?;
        Ok(ShadowsocksUdpPacket {
            target,
            port,
            payload: plain[payload_offset..].to_vec(),
        })
    }
}

/// Codec state for a Shadowsocks UDP datagram chain hop.
///
/// Captures the cipher and password needed to encode/decode Shadowsocks
/// UDP datagrams in a relay chain. Implements [`DatagramCodec`] from
/// `zero-traits` so the proxy runtime can use it without protocol-specific
/// adapter code.
#[cfg(feature = "crypto")]
#[derive(Debug, Clone)]
pub struct ShadowsocksDatagramCodec {
    pub cipher: super::shared::CipherKind,
    pub password: alloc::vec::Vec<u8>,
}

#[cfg(feature = "crypto")]
impl DatagramCodec<Address> for ShadowsocksDatagramCodec {
    type Error = Error;

    fn encode(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<alloc::vec::Vec<u8>, Self::Error> {
        <ShadowsocksOutbound as UdpDatagramFraming<
            ShadowsocksUdpPacketTarget<'_>,
            ShadowsocksUdpDecodeContext<'_>,
        >>::encode_udp_datagram(
            &ShadowsocksOutbound,
            &ShadowsocksUdpPacketTarget {
                target,
                port,
                payload,
                cipher: self.cipher,
                password: &self.password,
            },
        )
    }

    fn decode(&self, data: &[u8]) -> Option<(Address, u16, alloc::vec::Vec<u8>)> {
        let decoded = <ShadowsocksOutbound as UdpDatagramFraming<
            ShadowsocksUdpPacketTarget<'_>,
            ShadowsocksUdpDecodeContext<'_>,
        >>::decode_udp_datagram(
            &ShadowsocksOutbound,
            &ShadowsocksUdpDecodeContext {
                cipher: self.cipher,
                password: &self.password,
            },
            data,
        )
        .ok()?;
        Some((decoded.target, decoded.port, decoded.payload))
    }
}
