use zero_core::{Address, Error, Network, ProtocolType, Session, SessionAuth};
use zero_traits::AsyncSocket;

use crate::crypto::{
    aead_decrypt, aead_encrypt, compute_auth_id, current_timestamp, derive_body_key_nonce,
    derive_cmd_key, derive_response_key_nonce, hex, GCM_TAG_LEN,
};
use crate::shared::{read_exact, VmessCipher, AUTH_ID_LEN, CMD_TCP, CMD_UDP, VERSION};

#[derive(Clone)]
pub struct VmessUser {
    pub id: [u8; 16],
    pub cipher: VmessCipher,
    pub credential_id: Option<String>,
    pub principal_key: Option<String>,
    pub up_bps: Option<u64>,
    pub down_bps: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
pub struct VmessInbound;

impl VmessInbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Vmess
    }

    /// Accept with a single known user.
    pub async fn accept_tcp_with_auth<S: AsyncSocket>(
        &self,
        stream: &mut S,
        user: &VmessUser,
    ) -> Result<Session, Error> {
        let buf = VmessReadBuffer::read(stream).await?;
        let (session, body_key, auth_id, cipher) = try_user(&buf, user)?;
        send_auth_response(stream, &body_key, &auth_id, cipher).await?;
        Ok(session)
    }

    /// Read the wire auth packet once, then try each user in order.
    /// Returns the session built from the first user whose key successfully
    /// decrypts and verifies the auth header.
    pub async fn accept_tcp_with_auth_multi<S: AsyncSocket>(
        &self,
        stream: &mut S,
        users: &[VmessUser],
    ) -> Result<Session, Error> {
        let buf = VmessReadBuffer::read(stream).await?;

        for user in users {
            match try_user(&buf, user) {
                Ok((session, body_key, auth_id, cipher)) => {
                    return send_auth_response(stream, &body_key, &auth_id, cipher)
                        .await
                        .map(|()| session);
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
    encrypted: Vec<u8>,
}

impl VmessReadBuffer {
    async fn read<S: AsyncSocket>(stream: &mut S) -> Result<Self, Error> {
        let mut auth_id = [0u8; AUTH_ID_LEN];
        read_exact(stream, &mut auth_id).await?;

        let mut len_buf = [0u8; 2];
        read_exact(stream, &mut len_buf).await?;
        let body_len = u16::from_be_bytes(len_buf) as usize;
        if body_len > 2048 {
            return Err(Error::Protocol("vmess body too large"));
        }

        let mut encrypted = vec![0u8; body_len + GCM_TAG_LEN];
        read_exact(stream, &mut encrypted).await?;

        Ok(Self { auth_id, encrypted })
    }
}

/// Try one user against the buffered wire data.
/// Returns (session, body_key, auth_id, cipher) on success so the caller
/// can send the response through the live stream.
fn try_user(
    buf: &VmessReadBuffer,
    user: &VmessUser,
) -> Result<(Session, Vec<u8>, [u8; AUTH_ID_LEN], VmessCipher), Error> {
    let cmd_key = derive_cmd_key(&user.id, user.cipher.key_len());

    let (body_key_bytes, body_nonce_bytes) =
        derive_body_key_nonce(&cmd_key, &buf.auth_id, user.cipher.key_len());

    let plaintext = aead_decrypt(
        &body_key_bytes,
        &body_nonce_bytes,
        &buf.encrypted,
        user.cipher,
    )?;

    let (timestamp, session) = parse_command_body(&plaintext, &user.id)?;

    let expected_auth = compute_auth_id(&cmd_key, timestamp);
    if buf.auth_id != expected_auth {
        return Err(Error::Protocol("vmess auth verification failed"));
    }

    let now = current_timestamp();
    let delta = timestamp.abs_diff(now);
    if delta > 120 {
        return Err(Error::Protocol("vmess timestamp expired"));
    }

    Ok((session, body_key_bytes, buf.auth_id, user.cipher))
}

/// Parse the decrypted command body:
/// [timestamp:8][random:4][options:1][padding_len:1][padding:P][version:1][cmd:1][port:2][atyp:1][address:var]
fn parse_command_body(plaintext: &[u8], uuid: &[u8; 16]) -> Result<(u64, Session), Error> {
    if plaintext.len() < 20 {
        return Err(Error::Protocol("vmess body too short"));
    }

    let timestamp = u64::from_be_bytes(plaintext[0..8].try_into().unwrap());
    let _random = &plaintext[8..12];
    let _options = plaintext[12];
    let padding_len = plaintext[13] as usize;

    let cmd_start = 14 + padding_len;
    if plaintext.len() < cmd_start + 5 {
        return Err(Error::Protocol("vmess command section too short"));
    }

    let version = plaintext[cmd_start];
    if version != VERSION {
        return Err(Error::Protocol("vmess unsupported version"));
    }

    let command = plaintext[cmd_start + 1];
    let port = u16::from_be_bytes(plaintext[cmd_start + 2..cmd_start + 4].try_into().unwrap());
    let atyp = plaintext[cmd_start + 4];

    // Read address from remaining bytes
    let addr_len = match atyp {
        0x01 => 4usize, // IPv4
        0x02 => {
            if plaintext.len() <= cmd_start + 6 {
                return Err(Error::Protocol("vmess domain address truncated"));
            }
            1 + plaintext[cmd_start + 5] as usize // 1 byte len + domain
        }
        0x03 => 16usize, // IPv6
        _ => return Err(Error::Protocol("vmess unknown address type")),
    };

    let addr_start = cmd_start + 5;
    let addr_end = addr_start + addr_len;
    if plaintext.len() < addr_end {
        return Err(Error::Protocol("vmess address truncated"));
    }

    let target = parse_address_from_bytes(atyp, &plaintext[addr_start..addr_end])?;

    let network = match command {
        CMD_TCP => Network::Tcp,
        CMD_UDP => Network::Udp,
        _ => return Err(Error::Protocol("vmess unsupported command")),
    };

    let mut session = Session::new(0, target, port, network, ProtocolType::Vmess);

    let auth = SessionAuth {
        scheme: "vmess-uuid".into(),
        credential_id: None,
        principal_key: Some(hex::encode(uuid)),
        up_bps: None,
        down_bps: None,
    };
    session.apply_auth(auth);

    Ok((timestamp, session))
}

fn parse_address_from_bytes(atyp: u8, bytes: &[u8]) -> Result<Address, Error> {
    match atyp {
        0x01 => {
            let addr: [u8; 4] = bytes[..4].try_into().unwrap();
            Ok(Address::Ipv4(addr))
        }
        0x02 => {
            let len = bytes[0] as usize;
            let domain = std::str::from_utf8(&bytes[1..1 + len])
                .map_err(|_| Error::Protocol("vmess domain not utf-8"))?;
            Ok(Address::Domain(domain.to_owned()))
        }
        0x03 => {
            let addr: [u8; 16] = bytes[..16].try_into().unwrap();
            Ok(Address::Ipv6(addr))
        }
        _ => Err(Error::Protocol("vmess unexpected address type")),
    }
}

async fn send_auth_response<S: AsyncSocket>(
    stream: &mut S,
    body_key: &[u8],
    auth_id: &[u8; 16],
    cipher: VmessCipher,
) -> Result<(), Error> {
    let (resp_key, resp_nonce) = derive_response_key_nonce(body_key, auth_id, cipher.key_len());

    // Plaintext: [status:1] = 0x00 (success)
    let plaintext = [0x00u8];
    let encrypted = aead_encrypt(&resp_key, &resp_nonce, &plaintext, cipher)?;

    // Send: [len:2 BE][encrypted_body][tag]
    let body_len = (encrypted.len() - GCM_TAG_LEN) as u16;
    let mut buf = Vec::with_capacity(2 + encrypted.len());
    buf.extend_from_slice(&body_len.to_be_bytes());
    buf.extend_from_slice(&encrypted);

    stream
        .write_all(&buf)
        .await
        .map_err(|_| Error::Io("vmess: failed to write response"))?;

    Ok(())
}
