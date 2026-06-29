use rand::Rng;
use zero_core::{Error, ProtocolType, Session};
use zero_traits::{AsyncSocket, TcpSessionProtocol};

use crate::crypto::{
    create_xray_auth_id, current_timestamp, derive_xray_cmd_key, seal_xray_aead_header,
};
use crate::shared::{parse_uuid, VmessCipher, CMD_TCP, VERSION};
use crate::stream::VmessAeadStream;

#[derive(Debug, Clone, Copy)]
pub struct VmessOutbound;

pub struct VmessOutboundSession {
    pub upload_key: Vec<u8>,
    pub upload_nonce: Vec<u8>,
    pub download_key: Vec<u8>,
    pub download_nonce: Vec<u8>,
    pub cipher: VmessCipher,
    pub authenticated_length: bool,
    pub chunk_masking: bool,
    pub global_padding: bool,
    pub length_key_source: Vec<u8>,
    pub length_nonce_source: Vec<u8>,
    pub response_header: Option<u8>,
}

struct PendingVmessSession {
    response_header: u8,
    request_key: Vec<u8>,
    request_nonce: Vec<u8>,
    response_key: Vec<u8>,
    response_nonce: Vec<u8>,
    cipher: VmessCipher,
    authenticated_length: bool,
    chunk_masking: bool,
    global_padding: bool,
}

impl PendingVmessSession {
    fn into_session(self) -> VmessOutboundSession {
        let length_key_source = self.request_key.clone();
        let length_nonce_source = self.request_nonce.clone();
        VmessOutboundSession {
            upload_key: self.request_key,
            upload_nonce: self.request_nonce,
            download_key: self.response_key,
            download_nonce: self.response_nonce,
            cipher: self.cipher,
            authenticated_length: self.authenticated_length,
            chunk_masking: self.chunk_masking,
            global_padding: self.global_padding,
            length_key_source,
            length_nonce_source,
            response_header: Some(self.response_header),
        }
    }
}

impl VmessOutbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Vmess
    }

    pub async fn send_tcp_request<S: AsyncSocket>(
        &self,
        stream: &mut S,
        session: &Session,
        uuid: &[u8; 16],
        cipher: VmessCipher,
    ) -> Result<(), Error> {
        send_request(stream, session, uuid, cipher, CMD_TCP)
            .await
            .map(|_| ())
    }

    pub async fn establish_tcp_session<S: AsyncSocket>(
        &self,
        stream: &mut S,
        session: &Session,
        uuid: &[u8; 16],
        cipher: VmessCipher,
    ) -> Result<VmessOutboundSession, Error> {
        let pending = send_request(stream, session, uuid, cipher, CMD_TCP).await?;
        Ok(pending.into_session())
    }

    pub async fn establish_command_session<S: AsyncSocket>(
        &self,
        stream: &mut S,
        session: &Session,
        uuid: &[u8; 16],
        cipher: VmessCipher,
        command: u8,
    ) -> Result<VmessOutboundSession, Error> {
        let pending = send_request(stream, session, uuid, cipher, command).await?;
        Ok(pending.into_session())
    }
}

/// Target parameters for VMess TCP session.
#[derive(Debug, Clone, Copy)]
pub struct VmessTcpSessionTarget<'a> {
    pub session: &'a Session,
    pub uuid: &'a [u8; 16],
    pub cipher: VmessCipher,
}

/// Parsed VMess identity settings built from external config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmessTcpConnectConfig {
    uuid: [u8; 16],
    cipher_name: String,
    cipher: VmessCipher,
}

impl VmessTcpConnectConfig {
    pub fn from_config(id: &str, cipher: &str) -> Result<Self, Error> {
        let uuid = parse_uuid(id)?;
        let cipher =
            VmessCipher::from_name(cipher).ok_or(Error::Protocol("vmess unknown cipher"))?;
        Ok(Self {
            uuid,
            cipher_name: cipher.name().to_owned(),
            cipher,
        })
    }

    pub fn mux_pool_identity(&self) -> crate::mux::VmessMuxIdentity {
        crate::mux::VmessMuxIdentity::from_parts(self.uuid, self.cipher_name.clone(), self.cipher)
    }

    pub fn tcp_target<'a>(&'a self, session: &'a Session) -> VmessTcpSessionTarget<'a> {
        VmessTcpSessionTarget {
            session,
            uuid: &self.uuid,
            cipher: self.cipher,
        }
    }

    pub async fn establish_tcp_outbound_session<S>(
        &self,
        stream: &mut S,
        session: &Session,
    ) -> Result<VmessOutboundSession, Error>
    where
        S: AsyncSocket,
    {
        VmessOutbound
            .establish_tcp_session(stream, session, &self.uuid, self.cipher)
            .await
    }

    pub async fn establish_tcp_outbound_stream<S>(
        &self,
        mut stream: S,
        session: &Session,
    ) -> Result<VmessAeadStream<S>, Error>
    where
        S: AsyncSocket,
    {
        let vmess_session = self
            .establish_tcp_outbound_session(&mut stream, session)
            .await?;
        self.wrap_tcp_outbound_stream(stream, vmess_session)
    }

    pub fn wrap_tcp_outbound_stream<S>(
        &self,
        stream: S,
        session: VmessOutboundSession,
    ) -> Result<VmessAeadStream<S>, Error> {
        VmessAeadStream::outbound(stream, session)
    }
}

pub fn tcp_connect_config_from_config(
    id: &str,
    cipher: &str,
) -> Result<VmessTcpConnectConfig, Error> {
    VmessTcpConnectConfig::from_config(id, cipher)
}

impl<'a> TcpSessionProtocol<VmessTcpSessionTarget<'a>> for VmessOutbound {
    type Error = Error;
    type Session = VmessOutboundSession;

    async fn establish_tcp_session<S>(
        &self,
        stream: &mut S,
        target: &VmessTcpSessionTarget<'a>,
    ) -> Result<Self::Session, Self::Error>
    where
        S: AsyncSocket,
    {
        self.establish_tcp_session(stream, target.session, target.uuid, target.cipher)
            .await
    }
}

pub async fn establish_tcp_outbound_stream<S>(
    mut stream: S,
    session: &Session,
    uuid: &[u8; 16],
    cipher: VmessCipher,
) -> Result<VmessAeadStream<S>, Error>
where
    S: AsyncSocket,
{
    let vmess_session = VmessOutbound
        .establish_tcp_session(&mut stream, session, uuid, cipher)
        .await?;
    VmessAeadStream::outbound(stream, vmess_session)
}

pub async fn establish_tcp_outbound_session<S>(
    stream: &mut S,
    session: &Session,
    uuid: &[u8; 16],
    cipher: VmessCipher,
) -> Result<VmessOutboundSession, Error>
where
    S: AsyncSocket,
{
    VmessOutbound
        .establish_tcp_session(stream, session, uuid, cipher)
        .await
}

pub fn wrap_tcp_outbound_stream<S>(
    stream: S,
    session: VmessOutboundSession,
) -> Result<VmessAeadStream<S>, Error> {
    VmessAeadStream::outbound(stream, session)
}

async fn send_request<S: AsyncSocket>(
    stream: &mut S,
    session: &Session,
    uuid: &[u8; 16],
    cipher: VmessCipher,
    command: u8,
) -> Result<PendingVmessSession, Error> {
    let cmd_key = derive_xray_cmd_key(uuid);
    let timestamp = current_timestamp();
    let request_body_key = rand::rng().random::<[u8; 16]>();
    let request_body_nonce = rand::rng().random::<[u8; 16]>();
    let response_header = rand::rng().random::<u8>();

    let mut header = Vec::new();
    header.push(VERSION);
    header.extend_from_slice(&request_body_nonce);
    header.extend_from_slice(&request_body_key);
    header.push(response_header);
    let chunk_masking = false;
    let global_padding = false;
    let options = 0x01; // chunk stream
    header.push(options);
    let security = security_byte(cipher);
    header.push(security);
    header.push(0x00); // reserved
    header.push(command);
    if command != 0x03 {
        write_address_port_xray(&mut header, &session.target, session.port)?;
    }
    let checksum = fnv1a32(&header);
    header.extend_from_slice(&checksum.to_be_bytes());

    let auth_id = create_xray_auth_id(&cmd_key, timestamp)?;
    let packet = seal_xray_aead_header(&cmd_key, &auth_id, &header)?;

    stream
        .write_all(&packet)
        .await
        .map_err(|_| Error::Io("vmess: failed to write to socket"))?;

    let response_key = sha256_16(&request_body_key);
    let response_nonce = sha256_16(&request_body_nonce);

    Ok(PendingVmessSession {
        response_header,
        request_key: request_body_key.to_vec(),
        request_nonce: request_body_nonce.to_vec(),
        response_key,
        response_nonce,
        cipher,
        authenticated_length: false,
        chunk_masking,
        global_padding,
    })
}

fn security_byte(cipher: VmessCipher) -> u8 {
    match cipher {
        VmessCipher::Aes128Gcm => 0x03,
        VmessCipher::Chacha20Poly1305 => 0x04,
        VmessCipher::None => 0x05,
        VmessCipher::Zero => 0x06,
    }
}

fn write_address_port_xray(
    buf: &mut Vec<u8>,
    address: &zero_core::Address,
    port: u16,
) -> Result<(), Error> {
    buf.extend_from_slice(&port.to_be_bytes());
    match address {
        zero_core::Address::Ipv4(addr) => {
            buf.push(0x01);
            buf.extend_from_slice(addr);
        }
        zero_core::Address::Domain(domain) => {
            let bytes = domain.as_bytes();
            if bytes.len() > 255 {
                return Err(Error::Protocol("vmess domain too long (>255)"));
            }
            buf.push(0x02);
            buf.push(bytes.len() as u8);
            buf.extend_from_slice(bytes);
        }
        zero_core::Address::Ipv6(addr) => {
            buf.push(0x03);
            buf.extend_from_slice(addr);
        }
    }
    Ok(())
}

fn fnv1a32(bytes: &[u8]) -> u32 {
    let mut hash = 0x811c9dc5_u32;
    for byte in bytes {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x01000193);
    }
    hash
}

fn sha256_16(bytes: &[u8; 16]) -> Vec<u8> {
    let digest = ring::digest::digest(&ring::digest::SHA256, bytes);
    digest.as_ref()[..16].to_vec()
}
