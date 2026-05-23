use ring::aead::{Aad, BoundKey, Nonce, NonceSequence, OpeningKey, SealingKey, UnboundKey};
use ring::hkdf::{KeyType, Salt, HKDF_SHA256};
use ring::hmac;
use zero_core::{Address, Error, Network, ProtocolType, Session, SessionAuth};
use zero_traits::AsyncSocket;

use crate::shared::{
    read_exact, AUTH_ID_LEN, CMD_TCP, CMD_UDP, GCM_TAG_LEN, VERSION, VmessCipher,
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

fn derive_cmd_key(uuid: &[u8; 16], key_len: usize) -> Vec<u8> {
    let salt = Salt::new(HKDF_SHA256, b"VMess AEAD KDF");
    let prk = salt.extract(uuid);
    let info: &[u8] = b"";
    let info_array = [info];
    let okm = prk
        .expand(&info_array, CmdKeyLen(key_len))
        .map_err(|_| ())
        .expect("hkdf expand should succeed for valid key_len");
    let mut key = vec![0u8; key_len];
    okm.fill(&mut key)
        .map_err(|_| ())
        .expect("hkdf fill should succeed");
    key
}

struct CmdKeyLen(usize);

impl KeyType for CmdKeyLen {
    fn len(&self) -> usize {
        self.0
    }
}

fn compute_auth_id(cmd_key: &[u8], timestamp: u64) -> [u8; 16] {
    let key = hmac::Key::new(hmac::HMAC_SHA256, cmd_key);
    let tag = hmac::sign(&key, &timestamp.to_be_bytes());
    let mut result = [0u8; 16];
    result.copy_from_slice(&tag.as_ref()[..16]);
    result
}

fn derive_body_key_nonce(
    cmd_key: &[u8],
    auth_id: &[u8; 16],
    key_len: usize,
) -> (Vec<u8>, Vec<u8>) {
    // body_key = HKDF(cmd_key, salt="VMess Body Key", info=auth_id, len=key_len)
    let body_key = {
        let salt = Salt::new(HKDF_SHA256, b"VMess Body Key");
        let prk = salt.extract(cmd_key);
        let auth_info = [auth_id.as_ref()];
        let okm = prk
            .expand(&auth_info, CmdKeyLen(key_len))
            .expect("hkdf expand body_key");
        let mut k = vec![0u8; key_len];
        okm.fill(&mut k).expect("hkdf fill body_key");
        k
    };

    // body_nonce = HKDF(cmd_key, salt="VMess Body Nonce", info=auth_id, len=12)
    let body_nonce = {
        let salt = Salt::new(HKDF_SHA256, b"VMess Body Nonce");
        let prk = salt.extract(cmd_key);
        let auth_info = [auth_id.as_ref()];
        let okm = prk
            .expand(&auth_info, CmdKeyLen(12))
            .expect("hkdf expand body_nonce");
        let mut n = vec![0u8; 12];
        okm.fill(&mut n).expect("hkdf fill body_nonce");
        n
    };

    (body_key, body_nonce)
}

fn derive_response_key_nonce(
    body_key: &[u8],
    auth_id: &[u8; 16],
    key_len: usize,
) -> (Vec<u8>, Vec<u8>) {
    // response_key = HKDF(body_key, salt="VMess Resp Key", info=auth_id, len=key_len)
    let resp_key = {
        let salt = Salt::new(HKDF_SHA256, b"VMess Resp Key");
        let prk = salt.extract(body_key);
        let auth_info = [auth_id.as_ref()];
        let okm = prk
            .expand(&auth_info, CmdKeyLen(key_len))
            .expect("hkdf expand resp_key");
        let mut k = vec![0u8; key_len];
        okm.fill(&mut k).expect("hkdf fill resp_key");
        k
    };

    // response_nonce = HKDF(body_key, salt="VMess Resp Nonce", info=auth_id, len=12)
    let resp_nonce = {
        let salt = Salt::new(HKDF_SHA256, b"VMess Resp Nonce");
        let prk = salt.extract(body_key);
        let auth_info = [auth_id.as_ref()];
        let okm = prk
            .expand(&auth_info, CmdKeyLen(12))
            .expect("hkdf expand resp_nonce");
        let mut n = vec![0u8; 12];
        okm.fill(&mut n).expect("hkdf fill resp_nonce");
        n
    };

    (resp_key, resp_nonce)
}

fn aead_decrypt(
    key: &[u8],
    nonce_bytes: &[u8],
    ciphertext: &[u8],
    cipher: VmessCipher,
) -> Result<Vec<u8>, Error> {
    let unbound = UnboundKey::new(cipher.aead_algorithm(), key)
        .map_err(|_| Error::Protocol("vmess invalid aead key"))?;
    let nonce = Nonce::assume_unique_for_key(
        nonce_bytes[..12].try_into().map_err(|_| {
            Error::Protocol("vmess invalid nonce length")
        })?,
    );
    let mut opening_key = OpeningKey::new(unbound, CountingNonce::new(nonce));
    let mut in_out = ciphertext.to_vec();
    let plaintext = opening_key
        .open_in_place(Aad::empty(), &mut in_out)
        .map_err(|_| Error::Protocol("vmess aead decryption failed"))?;
    Ok(plaintext.to_vec())
}

fn aead_encrypt(
    key: &[u8],
    nonce_bytes: &[u8],
    plaintext: &[u8],
    cipher: VmessCipher,
) -> Result<Vec<u8>, Error> {
    let unbound = UnboundKey::new(cipher.aead_algorithm(), key)
        .map_err(|_| Error::Protocol("vmess invalid aead key"))?;
    let nonce = Nonce::assume_unique_for_key(
        nonce_bytes[..12].try_into().map_err(|_| {
            Error::Protocol("vmess invalid nonce length")
        })?,
    );
    let mut sealing_key = SealingKey::new(unbound, CountingNonce::new(nonce));
    let mut buf = plaintext.to_vec();
    buf.reserve(GCM_TAG_LEN);
    sealing_key
        .seal_in_place_append_tag(Aad::empty(), &mut buf)
        .map_err(|_| Error::Protocol("vmess aead encryption failed"))?;
    Ok(buf)
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
    let port = u16::from_be_bytes(
        plaintext[cmd_start + 2..cmd_start + 4]
            .try_into()
            .unwrap(),
    );
    let atyp = plaintext[cmd_start + 4];

    // Read address from remaining bytes
    let addr_len = match atyp {
        0x01 => 4usize,  // IPv4
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
            // IPv4
            let addr: [u8; 4] = bytes[..4].try_into().unwrap();
            Ok(Address::Ipv4(addr))
        }
        0x02 => {
            // Domain: [len:1][domain:len]
            let len = bytes[0] as usize;
            let domain = std::str::from_utf8(&bytes[1..1 + len])
                .map_err(|_| Error::Protocol("vmess domain not utf-8"))?;
            Ok(Address::Domain(domain.to_owned()))
        }
        0x03 => {
            // IPv6
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
    let (resp_key, resp_nonce) =
        derive_response_key_nonce(body_key, auth_id, cipher.key_len());

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

fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

struct CountingNonce {
    nonce: [u8; 12],
}

impl CountingNonce {
    fn new(initial: Nonce) -> Self {
        let mut nonce = [0u8; 12];
        nonce.copy_from_slice(initial.as_ref());
        Self { nonce }
    }
}

// ring 0.17 NonceSequence - use the trait from ring
impl NonceSequence for CountingNonce {
    fn advance(&mut self) -> Result<Nonce, ring::error::Unspecified> {
        // For our use-case (single encrypt/decrypt), nonce stays the same
        Ok(Nonce::assume_unique_for_key(self.nonce))
    }
}

// Simple hex encoding for principal_key display
mod hex {
    pub fn encode(bytes: &[u8; 16]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
}
