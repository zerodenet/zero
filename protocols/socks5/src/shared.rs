use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use zero_core::{Address, Error, ProtocolType};
use zero_traits::AsyncSocket;

use crate::outbound::{Socks5OutboundAuth, Socks5UdpFlowResume};
use crate::udp::{Socks5UdpAssociationConfig, Socks5UdpAssociationTarget};

pub(crate) const SOCKS5_VERSION: u8 = 0x05;
pub(crate) const METHOD_NO_AUTH: u8 = 0x00;
pub(crate) const METHOD_USERNAME_PASSWORD: u8 = 0x02;
pub(crate) const METHOD_NOT_ACCEPTABLE: u8 = 0xff;
pub(crate) const USERPASS_VERSION: u8 = 0x01;
pub(crate) const USERPASS_STATUS_SUCCESS: u8 = 0x00;
pub(crate) const USERPASS_STATUS_FAILURE: u8 = 0x01;

pub(crate) const CMD_CONNECT: u8 = 0x01;
pub(crate) const CMD_UDP_ASSOCIATE: u8 = 0x03;

pub(crate) const ATYP_IPV4: u8 = 0x01;
pub(crate) const ATYP_DOMAIN: u8 = 0x03;
pub(crate) const ATYP_IPV6: u8 = 0x04;

pub(crate) const REP_SUCCEEDED: u8 = 0x00;
pub(crate) const REP_GENERAL_FAILURE: u8 = 0x01;
pub(crate) const REP_CONNECTION_NOT_ALLOWED: u8 = 0x02;
pub(crate) const REP_HOST_UNREACHABLE: u8 = 0x04;
pub(crate) const REP_COMMAND_NOT_SUPPORTED: u8 = 0x07;
pub(crate) const REP_ADDRESS_TYPE_NOT_SUPPORTED: u8 = 0x08;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Socks5Reply {
    Succeeded,
    GeneralFailure,
    ConnectionNotAllowed,
    HostUnreachable,
    CommandNotSupported,
    AddressTypeNotSupported,
}

impl Socks5Reply {
    pub(crate) fn code(self) -> u8 {
        match self {
            Self::Succeeded => REP_SUCCEEDED,
            Self::GeneralFailure => REP_GENERAL_FAILURE,
            Self::ConnectionNotAllowed => REP_CONNECTION_NOT_ALLOWED,
            Self::HostUnreachable => REP_HOST_UNREACHABLE,
            Self::CommandNotSupported => REP_COMMAND_NOT_SUPPORTED,
            Self::AddressTypeNotSupported => REP_ADDRESS_TYPE_NOT_SUPPORTED,
        }
    }
}

pub(crate) async fn write_reply<S>(stream: &mut S, reply: Socks5Reply) -> Result<(), Error>
where
    S: AsyncSocket,
{
    write_reply_with_address(stream, reply, &Address::Ipv4([0, 0, 0, 0]), 0).await
}

pub(crate) async fn write_reply_with_address<S>(
    stream: &mut S,
    reply: Socks5Reply,
    address: &Address,
    port: u16,
) -> Result<(), Error>
where
    S: AsyncSocket,
{
    let mut response = vec![SOCKS5_VERSION, reply.code(), 0x00];
    write_address(&mut response, address)?;
    response.extend_from_slice(&port.to_be_bytes());
    stream
        .write_all(&response)
        .await
        .map_err(|_| Error::Io("failed to write SOCKS5 response"))
}

pub(crate) async fn read_exact<S>(stream: &mut S, buf: &mut [u8]) -> Result<(), Error>
where
    S: AsyncSocket,
{
    let mut offset = 0;

    while offset < buf.len() {
        let read = stream
            .read(&mut buf[offset..])
            .await
            .map_err(|_| Error::Io("failed to read from socket"))?;

        if read == 0 {
            return Err(Error::Io("unexpected EOF while reading socket"));
        }

        offset += read;
    }

    Ok(())
}

pub(crate) async fn read_address<S>(stream: &mut S, atyp: u8) -> Result<Address, Error>
where
    S: AsyncSocket,
{
    match atyp {
        ATYP_IPV4 => {
            let mut bytes = [0_u8; 4];
            read_exact(stream, &mut bytes).await?;
            Ok(Address::Ipv4(bytes))
        }
        ATYP_DOMAIN => {
            let mut length = [0_u8; 1];
            read_exact(stream, &mut length).await?;

            let domain_length = length[0] as usize;
            if domain_length == 0 {
                return Err(Error::Protocol("SOCKS5 domain must not be empty"));
            }

            let mut domain = vec![0_u8; domain_length];
            read_exact(stream, &mut domain).await?;

            let domain = alloc::string::String::from_utf8(domain)
                .map_err(|_| Error::Protocol("SOCKS5 domain is not valid UTF-8"))?;
            Ok(Address::Domain(domain))
        }
        ATYP_IPV6 => {
            let mut bytes = [0_u8; 16];
            read_exact(stream, &mut bytes).await?;
            Ok(Address::Ipv6(bytes))
        }
        _ => Err(Error::Unsupported("SOCKS5 address type is not supported")),
    }
}

pub(crate) fn write_address(buf: &mut Vec<u8>, address: &Address) -> Result<(), Error> {
    match address {
        Address::Ipv4(bytes) => {
            buf.push(ATYP_IPV4);
            buf.extend_from_slice(bytes);
        }
        Address::Ipv6(bytes) => {
            buf.push(ATYP_IPV6);
            buf.extend_from_slice(bytes);
        }
        Address::Domain(domain) => {
            let bytes = domain.as_bytes();
            if bytes.is_empty() {
                return Err(Error::Protocol("SOCKS5 domain must not be empty"));
            }
            if bytes.len() > u8::MAX as usize {
                return Err(Error::Unsupported("SOCKS5 domain is too long"));
            }

            buf.push(ATYP_DOMAIN);
            buf.push(bytes.len() as u8);
            buf.extend_from_slice(bytes);
        }
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Socks5UdpPacket {
    target: Address,
    port: u16,
    payload: Vec<u8>,
}

impl Socks5UdpPacket {
    pub(crate) fn new(target: Address, port: u16, payload: Vec<u8>) -> Self {
        Self {
            target,
            port,
            payload,
        }
    }

    pub(crate) fn into_parts(self) -> (Address, u16, Vec<u8>) {
        (self.target, self.port, self.payload)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5InboundUdpRequest {
    target: Address,
    port: u16,
    payload: Vec<u8>,
    frame_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5InboundUdpDispatchParts {
    target: Address,
    port: u16,
    payload: Vec<u8>,
    client_session_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5InboundUdpDispatchView {
    parts: Socks5InboundUdpDispatchParts,
    protocol_overhead_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Socks5InboundUdpDispatchAction {
    LocalDns { domain: String },
    Dispatch(Socks5InboundUdpDispatchView),
}

impl Socks5InboundUdpDispatchView {
    pub fn protocol(&self) -> ProtocolType {
        self.parts.protocol()
    }

    pub fn into_parts(self) -> (Socks5InboundUdpDispatchParts, usize) {
        (self.parts, self.protocol_overhead_len)
    }

    pub fn pipe_parts(&self) -> (&Address, u16, &[u8], Option<u64>) {
        self.parts.pipe_parts()
    }

    pub fn record_protocol_overhead<F>(&self, session_id: u64, record: F)
    where
        F: FnOnce(u64, u64),
    {
        record(session_id, self.protocol_overhead_len as u64);
    }
}

impl Socks5InboundUdpDispatchParts {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Socks5
    }

    pub fn pipe_parts(&self) -> (&Address, u16, &[u8], Option<u64>) {
        (
            &self.target,
            self.port,
            &self.payload,
            self.client_session_id,
        )
    }

    pub fn into_parts(self) -> (Address, u16, Vec<u8>, Option<u64>) {
        (self.target, self.port, self.payload, self.client_session_id)
    }
}

impl Socks5InboundUdpRequest {
    pub(crate) fn from_packet(packet: Socks5UdpPacket, frame_len: usize) -> Self {
        let (target, port, payload) = packet.into_parts();
        Self {
            target,
            port,
            payload,
            frame_len,
        }
    }

    pub fn target(&self) -> &Address {
        &self.target
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn is_dns_domain_request(&self) -> bool {
        matches!(&self.target, Address::Domain(_)) && self.port == 53
    }

    pub fn dns_domain_request(&self) -> Option<&str> {
        match (&self.target, self.port) {
            (Address::Domain(domain), 53) => Some(domain),
            _ => None,
        }
    }

    pub fn protocol_overhead_len(&self) -> usize {
        self.frame_len.saturating_sub(self.payload.len())
    }

    pub fn into_parts(self) -> (Address, u16, Vec<u8>) {
        (self.target, self.port, self.payload)
    }

    pub fn into_dispatch_parts(self) -> Socks5InboundUdpDispatchParts {
        let (target, port, payload) = self.into_parts();
        Socks5InboundUdpDispatchParts {
            target,
            port,
            payload,
            client_session_id: None,
        }
    }

    pub fn into_dispatch_action(self) -> Socks5InboundUdpDispatchAction {
        if let (Address::Domain(domain), 53) = (&self.target, self.port) {
            return Socks5InboundUdpDispatchAction::LocalDns {
                domain: domain.clone(),
            };
        }

        let protocol_overhead_len = self.protocol_overhead_len();
        Socks5InboundUdpDispatchAction::Dispatch(Socks5InboundUdpDispatchView {
            parts: self.into_dispatch_parts(),
            protocol_overhead_len,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5InboundUdpResponse {
    target: Address,
    port: u16,
    payload: Vec<u8>,
}

impl Socks5InboundUdpResponse {
    pub(crate) fn from_packet(packet: Socks5UdpPacket) -> Self {
        let (target, port, payload) = packet.into_parts();
        Self {
            target,
            port,
            payload,
        }
    }

    pub fn target(&self) -> &Address {
        &self.target
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn into_parts(self) -> (Address, u16, Vec<u8>) {
        (self.target, self.port, self.payload)
    }
}

pub(crate) fn parse_udp_packet(packet: &[u8]) -> Result<Socks5UdpPacket, Error> {
    if packet.len() < 4 {
        return Err(Error::Protocol("SOCKS5 UDP packet is too short"));
    }

    if packet[0] != 0 || packet[1] != 0 {
        return Err(Error::Protocol(
            "SOCKS5 UDP packet has invalid reserved bytes",
        ));
    }

    if packet[2] != 0 {
        return Err(Error::Unsupported(
            "SOCKS5 UDP fragmentation is not supported",
        ));
    }

    let atyp = packet[3];
    let mut offset = 4;

    let target = match atyp {
        ATYP_IPV4 => {
            if packet.len() < offset + 4 + 2 {
                return Err(Error::Protocol("SOCKS5 UDP IPv4 packet is truncated"));
            }
            let mut bytes = [0_u8; 4];
            bytes.copy_from_slice(&packet[offset..offset + 4]);
            offset += 4;
            Address::Ipv4(bytes)
        }
        ATYP_IPV6 => {
            if packet.len() < offset + 16 + 2 {
                return Err(Error::Protocol("SOCKS5 UDP IPv6 packet is truncated"));
            }
            let mut bytes = [0_u8; 16];
            bytes.copy_from_slice(&packet[offset..offset + 16]);
            offset += 16;
            Address::Ipv6(bytes)
        }
        ATYP_DOMAIN => {
            if packet.len() < offset + 1 {
                return Err(Error::Protocol("SOCKS5 UDP domain packet is truncated"));
            }
            let len = packet[offset] as usize;
            offset += 1;
            if len == 0 || packet.len() < offset + len + 2 {
                return Err(Error::Protocol("SOCKS5 UDP domain packet is truncated"));
            }
            let domain = alloc::string::String::from_utf8(packet[offset..offset + len].to_vec())
                .map_err(|_| Error::Protocol("SOCKS5 UDP domain is not valid UTF-8"))?;
            offset += len;
            Address::Domain(domain)
        }
        _ => {
            return Err(Error::Unsupported(
                "SOCKS5 UDP address type is not supported",
            ))
        }
    };

    let port = u16::from_be_bytes([packet[offset], packet[offset + 1]]);
    offset += 2;

    Ok(Socks5UdpPacket::new(
        target,
        port,
        packet[offset..].to_vec(),
    ))
}

pub(crate) fn build_udp_packet(
    address: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    let mut packet = vec![0_u8, 0_u8, 0_u8];
    write_address(&mut packet, address)?;
    packet.extend_from_slice(&port.to_be_bytes());
    packet.extend_from_slice(payload);
    Ok(packet)
}

pub(crate) fn decode_udp_associate_request(packet: &[u8]) -> Result<Socks5UdpPacket, Error> {
    parse_udp_packet(packet)
}

pub(crate) fn decode_udp_associate_response(packet: &[u8]) -> Result<Socks5UdpPacket, Error> {
    parse_udp_packet(packet)
}

pub(crate) fn encode_udp_associate_response(
    address: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    build_udp_packet(address, port, payload)
}

pub(crate) fn encode_udp_associate_response_to_client(
    upstream_address: &Address,
    upstream_port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    encode_udp_associate_response(upstream_address, upstream_port, payload)
}

fn udp_cache_key(tag: &str, server: &str, port: u16, username: Option<&str>) -> String {
    let auth = username
        .map(|value| alloc::format!("|auth:{value}"))
        .unwrap_or_default();
    alloc::format!("socks5|{tag}|{server}:{port}{auth}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Socks5UdpFlowConfig<'a> {
    tag: &'a str,
    server: &'a str,
    port: u16,
    username: Option<&'a str>,
    password: Option<&'a str>,
}

impl<'a> Socks5UdpFlowConfig<'a> {
    pub fn new(
        tag: &'a str,
        server: &'a str,
        port: u16,
        username: Option<&'a str>,
        password: Option<&'a str>,
    ) -> Self {
        Self {
            tag,
            server,
            port,
            username,
            password,
        }
    }

    pub fn flow_resume(&self) -> Socks5UdpFlowResume {
        Socks5UdpFlowResume::new(self.auth())
    }

    pub fn auth(&self) -> Option<Socks5OutboundAuth<'a>> {
        self.username
            .zip(self.password)
            .map(|(username, password)| Socks5OutboundAuth { username, password })
    }

    pub fn association_config(&self) -> Socks5UdpAssociationConfig<'a> {
        Socks5UdpAssociationConfig::new(self.auth())
    }

    pub fn cache_key(&self) -> String {
        udp_cache_key(self.tag, self.server, self.port, self.username)
    }

    pub fn association_target(&self) -> Socks5UdpAssociationTarget {
        self.flow_resume().association_target(
            self.tag.to_owned(),
            self.server.to_owned(),
            self.port,
        )
    }

    pub fn packet_path_spec(&self) -> Socks5UdpPacketPathSpec {
        Socks5UdpPacketPathSpec {
            cache_key: self.cache_key(),
            association_target: self.association_target(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5UdpPacketPathSpec {
    cache_key: String,
    association_target: Socks5UdpAssociationTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5UdpPacketPathCarrierBuild {
    cache_key: String,
    server: String,
    port: u16,
    association_target: Socks5UdpAssociationTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5UdpPacketPathCarrierDescriptor {
    cache_key: String,
    server: String,
    port: u16,
}

impl Socks5UdpPacketPathSpec {
    pub fn carrier_build(&self) -> Socks5UdpPacketPathCarrierBuild {
        Socks5UdpPacketPathCarrierBuild {
            cache_key: self.cache_key.clone(),
            server: self.association_target.server().to_owned(),
            port: self.association_target.port(),
            association_target: self.association_target.clone(),
        }
    }

    pub fn carrier_descriptor(&self) -> Socks5UdpPacketPathCarrierDescriptor {
        Socks5UdpPacketPathCarrierDescriptor {
            cache_key: self.cache_key.clone(),
            server: self.association_target.server().to_owned(),
            port: self.association_target.port(),
        }
    }
}

impl Socks5UdpPacketPathCarrierBuild {
    pub fn into_association_target(self) -> Socks5UdpAssociationTarget {
        self.association_target
    }
}

pub fn packet_path_carrier_association_target(
    carrier: Socks5UdpPacketPathCarrierBuild,
) -> Socks5UdpAssociationTarget {
    carrier.into_association_target()
}

impl Socks5UdpPacketPathCarrierDescriptor {
    pub fn into_parts(self) -> (String, String, u16) {
        (self.cache_key, self.server, self.port)
    }
}

pub fn udp_packet_path_spec_from_config(
    tag: &str,
    server: &str,
    port: u16,
    username: Option<&str>,
    password: Option<&str>,
) -> Socks5UdpPacketPathSpec {
    Socks5UdpFlowConfig::new(tag, server, port, username, password).packet_path_spec()
}

pub fn udp_packet_path_carrier_descriptor_from_config(
    tag: &str,
    server: &str,
    port: u16,
    username: Option<&str>,
    password: Option<&str>,
) -> Socks5UdpPacketPathCarrierDescriptor {
    udp_packet_path_spec_from_config(tag, server, port, username, password).carrier_descriptor()
}

pub fn udp_packet_path_carrier_build_from_config(
    tag: &str,
    server: &str,
    port: u16,
    username: Option<&str>,
    password: Option<&str>,
) -> Socks5UdpPacketPathCarrierBuild {
    udp_packet_path_spec_from_config(tag, server, port, username, password).carrier_build()
}

pub fn udp_flow_resume_from_config(
    tag: &str,
    server: &str,
    port: u16,
    username: Option<&str>,
    password: Option<&str>,
) -> Socks5UdpFlowResume {
    Socks5UdpFlowConfig::new(tag, server, port, username, password).flow_resume()
}
