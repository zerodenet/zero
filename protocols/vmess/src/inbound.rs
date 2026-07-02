use zero_core::{Error, Network, ProtocolType, Session, SessionAuth};
use zero_traits::AsyncSocket;

use crate::crypto::{
    derive_xray_cmd_key, hex, open_xray_aead_header_length, open_xray_aead_header_payload,
    seal_xray_response_header, GCM_TAG_LEN,
};
use crate::shared::{
    parse_address_from_bytes, parse_uuid, read_exact, VmessCipher, AUTH_ID_LEN, CMD_TCP, CMD_UDP,
    VERSION,
};

#[derive(Clone)]
pub struct VmessUser {
    pub id: [u8; 16],
    pub cipher: VmessCipher,
    pub credential_id: Option<String>,
    pub principal_key: Option<String>,
    pub up_bps: Option<u64>,
    pub down_bps: Option<u64>,
}

impl VmessUser {
    pub fn from_config(
        id: &str,
        cipher: &str,
        credential_id: Option<String>,
        principal_key: Option<String>,
        up_bps: Option<u64>,
        down_bps: Option<u64>,
    ) -> Result<Self, Error> {
        let id = parse_uuid(id)?;
        let cipher =
            VmessCipher::from_name(cipher).ok_or(Error::Protocol("vmess unknown cipher"))?;
        Ok(Self {
            id,
            cipher,
            credential_id,
            principal_key,
            up_bps,
            down_bps,
        })
    }
}

pub type VmessInboundUserConfigParts = (
    String,
    String,
    Option<String>,
    Option<String>,
    Option<u64>,
    Option<u64>,
);

pub type BorrowedVmessInboundUserConfigParts<'a> = (
    &'a str,
    &'a str,
    Option<&'a str>,
    Option<&'a str>,
    Option<u64>,
    Option<u64>,
);

#[derive(Clone)]
pub struct VmessInboundProfile {
    users: Vec<VmessUser>,
}

impl VmessInboundProfile {
    pub fn from_users(users: Vec<VmessUser>) -> Self {
        Self { users }
    }

    pub fn from_config_parts<I>(users: I) -> Result<Self, Error>
    where
        I: IntoIterator<Item = VmessInboundUserConfigParts>,
    {
        users
            .into_iter()
            .map(
                |(id, cipher, credential_id, principal_key, up_bps, down_bps)| {
                    VmessUser::from_config(
                        &id,
                        &cipher,
                        credential_id,
                        principal_key,
                        up_bps,
                        down_bps,
                    )
                },
            )
            .collect::<Result<Vec<_>, Error>>()
            .map(Self::from_users)
    }

    pub fn from_config_users<I, U>(users: I) -> Result<Self, Error>
    where
        I: IntoIterator<Item = U>,
        U: IntoVmessInboundUserConfig,
    {
        Self::from_config_parts(users.into_iter().map(U::into_vmess_inbound_user_config))
    }

    pub fn is_empty(&self) -> bool {
        self.users.is_empty()
    }

    pub async fn accept_tcp<S: AsyncSocket>(
        &self,
        inbound: VmessInbound,
        stream: &mut S,
    ) -> Result<VmessAccept, Error> {
        if self.users.len() == 1 {
            inbound.accept_tcp(stream, &self.users[0]).await
        } else {
            inbound.accept_tcp_multi(stream, &self.users).await
        }
    }

    pub async fn accept_tcp_stream<S: AsyncSocket>(
        &self,
        inbound: VmessInbound,
        mut stream: S,
    ) -> Result<(Session, crate::stream::VmessAeadStream<S>), Error> {
        let accepted = self.accept_tcp(inbound, &mut stream).await?;
        let session = accepted.session.clone();
        let client = crate::stream::wrap_tcp_inbound_stream(stream, accepted)?;
        Ok((session, client))
    }

    pub async fn accept_client<S: AsyncSocket>(
        &self,
        inbound: VmessInbound,
        stream: S,
    ) -> Result<crate::mux::VmessInboundAcceptedStream<crate::stream::VmessAeadStream<S>>, Error>
    {
        let (session, client) = self.accept_tcp_stream(inbound, stream).await?;
        Ok(crate::mux::VmessInboundAcceptedStream::from_session_stream(
            session, client,
        ))
    }
}

pub fn inbound_profile_from_config_users<I, U>(users: I) -> Result<VmessInboundProfile, Error>
where
    I: IntoIterator<Item = U>,
    U: IntoVmessInboundUserConfig,
{
    VmessInboundProfile::from_config_users(users)
}

pub trait IntoVmessInboundUserConfig {
    fn into_vmess_inbound_user_config(self) -> VmessInboundUserConfigParts;
}

impl IntoVmessInboundUserConfig for VmessInboundUserConfigParts {
    fn into_vmess_inbound_user_config(self) -> VmessInboundUserConfigParts {
        self
    }
}

impl IntoVmessInboundUserConfig for BorrowedVmessInboundUserConfigParts<'_> {
    fn into_vmess_inbound_user_config(self) -> VmessInboundUserConfigParts {
        let (id, cipher, credential_id, principal_key, up_bps, down_bps) = self;
        (
            id.to_owned(),
            cipher.to_owned(),
            credential_id.map(str::to_owned),
            principal_key.map(str::to_owned),
            up_bps,
            down_bps,
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct VmessInbound;

pub struct VmessAccept {
    pub session: Session,
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
    pub response_header: u8,
}

struct ParsedCommand {
    session: Session,
    body_key: Vec<u8>,
    body_nonce: Vec<u8>,
    response_header: u8,
    cipher: VmessCipher,
    authenticated_length: bool,
    chunk_masking: bool,
    global_padding: bool,
}

impl VmessInbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Vmess
    }

    pub fn udp_session(
        &self,
        default_target: zero_core::Address,
        default_port: u16,
    ) -> crate::udp::VmessInboundUdpSession {
        crate::udp::VmessInboundUdpSession::new(default_target, default_port)
    }

    pub fn udp_session_for(&self, session: &Session) -> crate::udp::VmessInboundUdpSession {
        self.udp_session(session.target.clone(), session.port)
    }

    pub fn udp_responder_for(&self, session: &Session) -> crate::udp::VmessInboundUdpResponder {
        crate::udp::VmessInboundUdpResponder::new(self.udp_session_for(session))
    }

    pub fn mux_udp_responder_for(
        &self,
        session: &Session,
        writer: crate::mux::VmessInboundMuxWriter,
        mux_session_id: u16,
    ) -> crate::udp::VmessInboundMuxUdpResponder {
        crate::udp::VmessInboundMuxUdpResponder::new(
            self.udp_session_for(session),
            writer,
            mux_session_id,
        )
    }

    /// Accept with a single known user.
    pub async fn accept_tcp_with_auth<S: AsyncSocket>(
        &self,
        stream: &mut S,
        user: &VmessUser,
    ) -> Result<Session, Error> {
        self.accept_tcp(stream, user)
            .await
            .map(|accepted| accepted.session)
    }

    pub async fn accept_tcp<S: AsyncSocket>(
        &self,
        stream: &mut S,
        user: &VmessUser,
    ) -> Result<VmessAccept, Error> {
        let buf = VmessReadBuffer::read(stream, std::slice::from_ref(user)).await?;
        let parsed = try_user(&buf, user)?;
        let (download_key, download_nonce) = send_auth_response(
            stream,
            &parsed.body_key,
            &parsed.body_nonce,
            parsed.response_header,
        )
        .await?;
        Ok(VmessAccept {
            session: parsed.session,
            upload_key: parsed.body_key.clone(),
            upload_nonce: parsed.body_nonce.clone(),
            download_key,
            download_nonce,
            cipher: parsed.cipher,
            authenticated_length: parsed.authenticated_length,
            chunk_masking: parsed.chunk_masking,
            global_padding: parsed.global_padding,
            length_key_source: parsed.body_key,
            length_nonce_source: parsed.body_nonce,
            response_header: parsed.response_header,
        })
    }

    /// Read the wire auth packet once, then try each user in order.
    /// Returns the session built from the first user whose key successfully
    /// decrypts and verifies the auth header.
    pub async fn accept_tcp_with_auth_multi<S: AsyncSocket>(
        &self,
        stream: &mut S,
        users: &[VmessUser],
    ) -> Result<Session, Error> {
        self.accept_tcp_multi(stream, users)
            .await
            .map(|accepted| accepted.session)
    }

    pub async fn accept_tcp_multi<S: AsyncSocket>(
        &self,
        stream: &mut S,
        users: &[VmessUser],
    ) -> Result<VmessAccept, Error> {
        let buf = VmessReadBuffer::read(stream, users).await?;

        for user in users {
            match try_user(&buf, user) {
                Ok(parsed) => {
                    let (download_key, download_nonce) = send_auth_response(
                        stream,
                        &parsed.body_key,
                        &parsed.body_nonce,
                        parsed.response_header,
                    )
                    .await?;
                    return Ok(VmessAccept {
                        session: parsed.session,
                        upload_key: parsed.body_key.clone(),
                        upload_nonce: parsed.body_nonce.clone(),
                        download_key,
                        download_nonce,
                        cipher: parsed.cipher,
                        authenticated_length: parsed.authenticated_length,
                        chunk_masking: parsed.chunk_masking,
                        global_padding: parsed.global_padding,
                        length_key_source: parsed.body_key,
                        length_nonce_source: parsed.body_nonce,
                        response_header: parsed.response_header,
                    });
                }
                Err(_) => continue,
            }
        }

        // Send a generic rejection so the client gets a response
        let reject = [0x00u8, 0x00u8];
        let _ = stream.write_all(&reject).await;
        Err(Error::Protocol("vmess: no user matched"))
    }
}

/// Buffered wire data read once, tried against multiple users.
struct VmessReadBuffer {
    auth_id: [u8; AUTH_ID_LEN],
    nonce: [u8; 8],
    encrypted_payload: Vec<u8>,
}

impl VmessReadBuffer {
    async fn read<S: AsyncSocket>(stream: &mut S, users: &[VmessUser]) -> Result<Self, Error> {
        let mut auth_id = [0u8; AUTH_ID_LEN];
        read_exact(stream, &mut auth_id).await?;

        let mut encrypted_len = [0_u8; 18];
        read_exact(stream, &mut encrypted_len).await?;
        let mut nonce = [0_u8; 8];
        read_exact(stream, &mut nonce).await?;

        let mut header_len = None;
        for user in users {
            let cmd_key = derive_xray_cmd_key(&user.id);
            if let Ok(len) =
                open_xray_aead_header_length(&cmd_key, &auth_id, &encrypted_len, &nonce)
            {
                header_len = Some(len);
                break;
            }
        }

        let header_len = header_len.ok_or(Error::Protocol("vmess: no user matched"))?;
        let mut encrypted_payload = vec![0_u8; header_len + GCM_TAG_LEN];
        read_exact(stream, &mut encrypted_payload).await?;

        Ok(Self {
            auth_id,
            nonce,
            encrypted_payload,
        })
    }
}

/// Try one user against the buffered wire data.
/// Returns (session, body_key, auth_id, cipher) on success so the caller
/// can send the response through the live stream.
fn try_user(buf: &VmessReadBuffer, user: &VmessUser) -> Result<ParsedCommand, Error> {
    let cmd_key = derive_xray_cmd_key(&user.id);
    let plaintext =
        open_xray_aead_header_payload(&cmd_key, &buf.auth_id, &buf.nonce, &buf.encrypted_payload)?;
    parse_command_body(&plaintext, user)
}

/// Parse the decrypted command body.
fn parse_command_body(plaintext: &[u8], user: &VmessUser) -> Result<ParsedCommand, Error> {
    if plaintext.len() < 42 {
        return Err(Error::Protocol("vmess body too short"));
    }

    let version = plaintext[0];
    if version != VERSION {
        return Err(Error::Protocol("vmess unsupported version"));
    }

    let body_nonce = plaintext[1..17].to_vec();
    let body_key = plaintext[17..33].to_vec();
    let response_header = plaintext[33];
    let options = plaintext[34];
    let padding_len = (plaintext[35] >> 4) as usize;
    let cipher = match plaintext[35] & 0x0f {
        0x03 => VmessCipher::Aes128Gcm,
        0x04 => VmessCipher::Chacha20Poly1305,
        0x05 => VmessCipher::None,
        0x06 => VmessCipher::Zero,
        _ => user.cipher,
    };
    let command = plaintext[37];
    let mut pos = 38;

    let target = if command == 0x03 {
        zero_core::Address::Domain(crate::shared::MUX_COOL_DOMAIN.to_owned())
    } else {
        if plaintext.len() < pos + 3 {
            return Err(Error::Protocol("vmess command section too short"));
        }
        let port = u16::from_be_bytes([plaintext[pos], plaintext[pos + 1]]);
        pos += 2;
        let atyp = plaintext[pos];
        pos += 1;
        let addr_len = match atyp {
            0x01 => 4usize,
            0x02 => {
                if plaintext.len() <= pos {
                    return Err(Error::Protocol("vmess domain address truncated"));
                }
                1 + plaintext[pos] as usize
            }
            0x03 => 16usize,
            _ => return Err(Error::Protocol("vmess unknown address type")),
        };
        let addr_end = pos + addr_len;
        if plaintext.len() < addr_end {
            return Err(Error::Protocol("vmess address truncated"));
        }
        let target = parse_address_from_bytes(atyp, &plaintext[pos..addr_end])?;
        let network = match command {
            CMD_TCP => Network::Tcp,
            CMD_UDP => Network::Udp,
            _ => return Err(Error::Protocol("vmess unsupported command")),
        };
        let mut session = Session::new(0, target, port, network, ProtocolType::Vmess);
        apply_user_auth(&mut session, user);
        return Ok(ParsedCommand {
            session,
            body_key,
            body_nonce,
            response_header,
            cipher,
            authenticated_length: options & 0x10 != 0,
            chunk_masking: options & 0x04 != 0,
            global_padding: options & 0x08 != 0,
        });
    };

    let mut session = Session::new(
        0,
        target,
        crate::shared::MUX_COOL_PORT,
        Network::Tcp,
        ProtocolType::Vmess,
    );
    apply_user_auth(&mut session, user);
    let _ = padding_len;
    Ok(ParsedCommand {
        session,
        body_key,
        body_nonce,
        response_header,
        cipher,
        authenticated_length: options & 0x10 != 0,
        chunk_masking: options & 0x04 != 0,
        global_padding: options & 0x08 != 0,
    })
}

fn apply_user_auth(session: &mut Session, user: &VmessUser) {
    let auth = SessionAuth {
        scheme: "vmess-uuid".into(),
        credential_id: user.credential_id.clone(),
        principal_key: user
            .principal_key
            .clone()
            .or_else(|| Some(hex::encode(&user.id))),
        up_bps: user.up_bps,
        down_bps: user.down_bps,
    };
    session.apply_auth(auth);
}

async fn send_auth_response<S: AsyncSocket>(
    stream: &mut S,
    body_key: &[u8],
    body_nonce: &[u8],
    response_header: u8,
) -> Result<(Vec<u8>, Vec<u8>), Error> {
    let resp_key = sha256_16(body_key)?;
    let resp_nonce = sha256_16(body_nonce)?;

    let plaintext = [response_header, 0x00, 0x00, 0x00];
    let buf = seal_xray_response_header(&resp_key, &resp_nonce, &plaintext)?;

    stream
        .write_all(&buf)
        .await
        .map_err(|_| Error::Io("vmess: failed to write response"))?;

    Ok((resp_key, resp_nonce))
}

fn sha256_16(bytes: &[u8]) -> Result<Vec<u8>, Error> {
    let digest = ring::digest::digest(&ring::digest::SHA256, bytes);
    Ok(digest.as_ref()[..16].to_vec())
}
