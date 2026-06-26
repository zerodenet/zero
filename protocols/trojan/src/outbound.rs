//! Trojan outbound protocol handler.

use zero_core::{Address, Error, ProtocolType, Session};
use zero_traits::{
    AsyncSocket, TcpTunnelProtocol, UdpPacketStreamFraming, UdpPacketTunnelProtocol,
};

use super::shared::{CMD_TCP, CMD_UDP};

/// Trojan outbound handler.
#[derive(Debug, Default, Clone, Copy)]
pub struct TrojanOutbound;

impl TrojanOutbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Trojan
    }

    /// Send the Trojan request over an established TLS stream.
    ///
    /// Writes: password hash + CRLF + CMD + address + port + CRLF.
    /// The upstream server then connects to the target and relays data.
    pub async fn send_request<S: AsyncSocket>(
        &self,
        stream: &mut S,
        session: &Session,
        password: &str,
    ) -> Result<(), Error> {
        let request = build_tcp_request(password, &session.target, session.port)?;
        stream
            .write_all(&request)
            .await
            .map_err(|_| Error::Io("trojan: write failed"))
    }
}

/// Target parameters for Trojan TCP tunnel.
#[derive(Debug, Clone, Copy)]
pub struct TrojanTcpTunnelTarget<'a> {
    pub session: &'a Session,
    pub password: &'a str,
}

impl<'a> TcpTunnelProtocol<TrojanTcpTunnelTarget<'a>> for TrojanOutbound {
    type Error = Error;

    async fn establish_tcp_tunnel<S>(
        &self,
        stream: &mut S,
        target: &TrojanTcpTunnelTarget<'a>,
    ) -> Result<(), Self::Error>
    where
        S: AsyncSocket,
    {
        self.send_request(stream, target.session, target.password)
            .await
    }
}

/// Target parameters for Trojan UDP packet tunnel over a connected stream.
#[derive(Debug, Clone, Copy)]
pub struct TrojanUdpPacketTunnelTarget<'a> {
    pub session: &'a Session,
    pub password: &'a str,
}

impl<'a> UdpPacketTunnelProtocol<TrojanUdpPacketTunnelTarget<'a>> for TrojanOutbound {
    type Error = Error;

    async fn establish_udp_packet_tunnel<S>(
        &self,
        stream: &mut S,
        target: &TrojanUdpPacketTunnelTarget<'a>,
    ) -> Result<(), Self::Error>
    where
        S: AsyncSocket,
    {
        let request =
            build_udp_request(target.password, &target.session.target, target.session.port)?;
        stream
            .write_all(&request)
            .await
            .map_err(|_| Error::Io("trojan: write udp request failed"))
    }
}

/// One Trojan UDP packet carried over a connected stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrojanUdpPacket {
    pub target: Address,
    pub port: u16,
    pub payload: Vec<u8>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TrojanUdpFlowIo;

impl TrojanUdpFlowIo {
    pub async fn establish<S>(
        &self,
        stream: &mut S,
        session: &Session,
        password: &str,
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        establish_udp_packet_tunnel(stream, session, password).await
    }

    pub async fn write_packet<S>(
        &self,
        stream: &mut S,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        write_udp_flow_packet(stream, target, port, payload).await
    }

    pub async fn read_packet<S>(&self, stream: &mut S) -> Result<TrojanUdpPacket, Error>
    where
        S: AsyncSocket,
    {
        read_udp_flow_packet(stream).await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrojanUdpFlowResume {
    password: String,
    sni: Option<String>,
    insecure: bool,
    client_fingerprint: Option<String>,
    relay_chain: bool,
}

impl TrojanUdpFlowResume {
    pub fn new(
        password: &str,
        sni: Option<&str>,
        insecure: bool,
        client_fingerprint: Option<&str>,
        relay_chain: bool,
    ) -> Self {
        Self {
            password: password.to_owned(),
            sni: sni.map(ToOwned::to_owned),
            insecure,
            client_fingerprint: client_fingerprint.map(ToOwned::to_owned),
            relay_chain,
        }
    }

    pub fn password(&self) -> &str {
        &self.password
    }

    pub fn sni(&self) -> Option<&str> {
        self.sni.as_deref()
    }

    pub fn insecure(&self) -> bool {
        self.insecure
    }

    pub fn client_fingerprint(&self) -> Option<&str> {
        self.client_fingerprint.as_deref()
    }

    pub fn relay_chain(&self) -> bool {
        self.relay_chain
    }

    pub fn peer_config(&self) -> TrojanUdpPeerConfig<'_> {
        TrojanUdpPeerConfig {
            password: &self.password,
            sni: self.sni.as_deref(),
            insecure: self.insecure,
            client_fingerprint: self.client_fingerprint.as_deref(),
            relay_chain: self.relay_chain,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TrojanUdpPeerConfig<'a> {
    password: &'a str,
    sni: Option<&'a str>,
    insecure: bool,
    client_fingerprint: Option<&'a str>,
    relay_chain: bool,
}

impl<'a> TrojanUdpPeerConfig<'a> {
    pub fn password(&self) -> &'a str {
        self.password
    }

    pub fn sni(&self) -> Option<&'a str> {
        self.sni
    }

    pub fn insecure(&self) -> bool {
        self.insecure
    }

    pub fn client_fingerprint(&self) -> Option<&'a str> {
        self.client_fingerprint
    }

    pub fn relay_chain(&self) -> bool {
        self.relay_chain
    }

    pub fn leaf_cache_key(&self, server: &str, port: u16) -> TrojanUdpLeafKey {
        TrojanUdpLeafKey {
            server: server.to_owned(),
            port,
            password: self.password.to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TrojanUdpLeafKey {
    server: String,
    port: u16,
    password: String,
}

impl TrojanUdpLeafKey {
    pub fn server(&self) -> &str {
        &self.server
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

impl UdpPacketStreamFraming<TrojanUdpPacket> for TrojanOutbound {
    type Error = Error;
    type Decoded = TrojanUdpPacket;

    async fn write_udp_packet<S>(
        &self,
        stream: &mut S,
        packet: &TrojanUdpPacket,
    ) -> Result<(), Self::Error>
    where
        S: AsyncSocket,
    {
        super::shared::write_udp_packet(stream, &packet.target, packet.port, &packet.payload).await
    }

    async fn read_udp_packet<S>(&self, stream: &mut S) -> Result<Self::Decoded, Self::Error>
    where
        S: AsyncSocket,
    {
        let (target, port, payload) = super::shared::read_udp_packet(stream).await?;
        Ok(TrojanUdpPacket {
            target,
            port,
            payload,
        })
    }
}

pub async fn read_inbound_udp_packet<S>(stream: &mut S) -> Result<TrojanUdpPacket, Error>
where
    S: AsyncSocket,
{
    <TrojanOutbound as UdpPacketStreamFraming<TrojanUdpPacket>>::read_udp_packet(
        &TrojanOutbound,
        stream,
    )
    .await
}

pub async fn read_udp_flow_packet<S>(stream: &mut S) -> Result<TrojanUdpPacket, Error>
where
    S: AsyncSocket,
{
    read_inbound_udp_packet(stream).await
}

pub async fn establish_udp_packet_tunnel<S>(
    stream: &mut S,
    session: &Session,
    password: &str,
) -> Result<(), Error>
where
    S: AsyncSocket,
{
    <TrojanOutbound as UdpPacketTunnelProtocol<TrojanUdpPacketTunnelTarget<'_>>>::establish_udp_packet_tunnel(
        &TrojanOutbound,
        stream,
        &TrojanUdpPacketTunnelTarget { session, password },
    )
    .await
}

pub async fn write_udp_response<S>(
    stream: &mut S,
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<(), Error>
where
    S: AsyncSocket,
{
    let packet = TrojanUdpPacket {
        target: target.clone(),
        port,
        payload: payload.to_vec(),
    };
    <TrojanOutbound as UdpPacketStreamFraming<TrojanUdpPacket>>::write_udp_packet(
        &TrojanOutbound,
        stream,
        &packet,
    )
    .await
}

pub async fn write_udp_flow_packet<S>(
    stream: &mut S,
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<(), Error>
where
    S: AsyncSocket,
{
    write_udp_response(stream, target, port, payload).await
}

/// Build a Trojan UDP associate request (CMD_UDP).
///
/// This is a standalone request builder used by the proxy outbound
/// module to initiate a UDP relay connection.
pub fn build_udp_request(password: &str, addr: &Address, port: u16) -> Result<Vec<u8>, Error> {
    build_trojan_request(password, addr, port, CMD_UDP)
}

fn build_tcp_request(password: &str, addr: &Address, port: u16) -> Result<Vec<u8>, Error> {
    build_trojan_request(password, addr, port, CMD_TCP)
}

fn build_trojan_request(
    password: &str,
    addr: &Address,
    port: u16,
    cmd: u8,
) -> Result<Vec<u8>, Error> {
    use super::shared::{ATYP_DOMAIN, ATYP_IPV4, ATYP_IPV6, CRLF};

    let mut request = Vec::new();

    #[cfg(feature = "crypto")]
    {
        use sha2::{Digest, Sha224};
        let digest = Sha224::digest(password.as_bytes());
        request.extend_from_slice(super::shared::hex::encode(&digest).as_bytes());
    }
    #[cfg(not(feature = "crypto"))]
    {
        let _ = password;
        return Err(Error::Unsupported("trojan: crypto feature not enabled"));
    }

    request.extend_from_slice(CRLF);
    request.push(cmd);

    match addr {
        Address::Ipv4(bytes) => {
            request.push(ATYP_IPV4);
            request.extend_from_slice(bytes);
        }
        Address::Ipv6(bytes) => {
            request.push(ATYP_IPV6);
            request.extend_from_slice(bytes);
        }
        Address::Domain(domain) => {
            let bytes = domain.as_bytes();
            if bytes.is_empty() || bytes.len() > 255 {
                return Err(Error::Protocol("trojan: domain too long"));
            }
            request.push(ATYP_DOMAIN);
            request.push(bytes.len() as u8);
            request.extend_from_slice(bytes);
        }
    }

    request.extend_from_slice(&port.to_be_bytes());
    request.extend_from_slice(CRLF);
    Ok(request)
}
