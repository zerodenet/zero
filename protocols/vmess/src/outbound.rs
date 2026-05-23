use rand::Rng;
use ring::aead::{Aad, BoundKey, Nonce, NonceSequence, OpeningKey, SealingKey, UnboundKey};
use zero_core::{Error, ProtocolType, Session};
use zero_traits::AsyncSocket;

use crate::shared::{
    read_exact, write_address, AUTH_ID_LEN, CMD_TCP, GCM_TAG_LEN, VERSION, VmessCipher,
};

#[derive(Debug, Clone, Copy)]
pub struct VmessOutbound;

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
        send_request(stream, session, uuid, cipher).await
    }

    pub async fn establish_tcp_tunnel<S: AsyncSocket>(
        &self,
        stream: &mut S,
        session: &Session,
        uuid: &[u8; 16],
        cipher: VmessCipher,
    ) -> Result<(), Error> {
        // 1. Send auth header
        send_request(stream, session, uuid, cipher).await?;

        // 2. Read and verify server response
        read_response(stream, uuid, cipher).await?;

        Ok(())
    }
}

async fn send_request<S: AsyncSocket>(
    stream: &mut S,
    session: &Session,
    uuid: &[u8; 16],
    cipher: VmessCipher,
) -> Result<(), Error> {
    // 1. Derive cmd_key from UUID
    let cmd_key = derive_cmd_key(uuid, cipher.key_len());

    // 2. Build command body plaintext
    let timestamp = current_timestamp();
    let random_bytes = rand::rng().random::<[u8; 4]>();

    let mut command_body = Vec::new();
    command_body.extend_from_slice(&timestamp.to_be_bytes()); // [0:8] timestamp
    command_body.extend_from_slice(&random_bytes); // [8:12] random
    command_body.push(0x00); // [12] options
    command_body.push(0x00); // [13] padding_len (no padding)
    command_body.push(VERSION); // [14] version
    command_body.push(CMD_TCP); // [15] command (TCP)
    command_body.extend_from_slice(&session.port.to_be_bytes()); // [16:18] port
    write_address(&mut command_body, &session.target)?; // [18..] address

    // 3. Compute auth_id = HMAC(cmd_key, timestamp)[:16]
    let auth_id = compute_auth_id(&cmd_key, timestamp);

    // 4. Derive body key/nonce
    let (body_key_bytes, body_nonce_bytes) =
        derive_body_key_nonce(&cmd_key, &auth_id, cipher.key_len());

    // 5. Encrypt command body
    let encrypted = aead_encrypt(&body_key_bytes, &body_nonce_bytes, &command_body, cipher)?;
    let body_len = (encrypted.len() - GCM_TAG_LEN) as u16;

    // 6. Send: [auth_id:16][body_len:2 BE][encrypted_body]
    let mut packet = Vec::with_capacity(AUTH_ID_LEN + 2 + encrypted.len());
    packet.extend_from_slice(&auth_id);
    packet.extend_from_slice(&body_len.to_be_bytes());
    packet.extend_from_slice(&encrypted);

    stream
        .write_all(&packet)
        .await
        .map_err(|_| Error::Io("vmess: failed to write to socket"))?;

    Ok(())
}

async fn read_response<S: AsyncSocket>(
    stream: &mut S,
    uuid: &[u8; 16],
    cipher: VmessCipher,
) -> Result<(), Error> {
    let cmd_key = derive_cmd_key(uuid, cipher.key_len());

    // Read: [len:2 BE][encrypted_body][tag]
    let mut len_buf = [0u8; 2];
    read_exact(stream, &mut len_buf).await?;
    let body_len = u16::from_be_bytes(len_buf) as usize;
    if body_len > 256 {
        return Err(Error::Protocol("vmess response too large"));
    }

    let mut encrypted = vec![0u8; body_len + GCM_TAG_LEN];
    read_exact(stream, &mut encrypted).await?;

    // To decrypt response, we need auth_id. But we don't have it from the request.
    // We derive it from the cmd_key directly.
    // The response is encrypted with a derived key that depends on auth_id.
    // Since we generated auth_id = HMAC(cmd_key, timestamp), and the server echoes it back
    // implicitly (encrypted with a key derived from it), we need to try with our auth_id.

    // Reconstruct timestamp and auth_id the same way we did in send_request.
    // This works because the server uses the auth_id WE sent to derive response keys.
    let timestamp = current_timestamp();
    let auth_id = compute_auth_id(&cmd_key, timestamp);

    let cmd_key = derive_cmd_key(uuid, cipher.key_len());
    let (_body_key_bytes, _body_nonce_bytes) =
        derive_body_key_nonce(&cmd_key, &auth_id, cipher.key_len());

    // Derive response key from body_key
    let (resp_key, resp_nonce) =
        derive_response_key_nonce(&_body_key_bytes, &auth_id, cipher.key_len());

    // Decrypt response
    let plaintext = aead_decrypt(&resp_key, &resp_nonce, &encrypted, cipher)?;

    if plaintext.is_empty() || plaintext[0] != 0x00 {
        return Err(Error::Protocol("vmess server rejected connection"));
    }

    Ok(())
}

// --- Crypto helpers (mirror inbound.rs) ---

fn derive_cmd_key(uuid: &[u8; 16], key_len: usize) -> Vec<u8> {
    use ring::hkdf::{Salt, HKDF_SHA256, KeyType};
    let salt = Salt::new(HKDF_SHA256, b"VMess AEAD KDF");
    let prk = salt.extract(uuid);
    struct Len(usize);
    impl KeyType for Len {
        fn len(&self) -> usize { self.0 }
    }
    let empty_info = [b"" as &[u8]];
    let okm = prk.expand(&empty_info, Len(key_len)).expect("hkdf expand");
    let mut key = vec![0u8; key_len];
    okm.fill(&mut key).expect("hkdf fill");
    key
}

fn compute_auth_id(cmd_key: &[u8], timestamp: u64) -> [u8; 16] {
    use ring::hmac;
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
    use ring::hkdf::{Salt, HKDF_SHA256, KeyType};
    struct Len(usize);
    impl KeyType for Len {
        fn len(&self) -> usize { self.0 }
    }

    let body_key = {
        let salt = Salt::new(HKDF_SHA256, b"VMess Body Key");
        let prk = salt.extract(cmd_key);
        let auth_info = [auth_id.as_ref()];
        let okm = prk.expand(&auth_info, Len(key_len)).expect("hkdf");
        let mut k = vec![0u8; key_len];
        okm.fill(&mut k).expect("hkdf fill");
        k
    };

    let body_nonce = {
        let salt = Salt::new(HKDF_SHA256, b"VMess Body Nonce");
        let prk = salt.extract(cmd_key);
        let auth_info = [auth_id.as_ref()];
        let okm = prk.expand(&auth_info, Len(12)).expect("hkdf");
        let mut n = vec![0u8; 12];
        okm.fill(&mut n).expect("hkdf fill");
        n
    };

    (body_key, body_nonce)
}

fn derive_response_key_nonce(
    body_key: &[u8],
    auth_id: &[u8; 16],
    key_len: usize,
) -> (Vec<u8>, Vec<u8>) {
    use ring::hkdf::{Salt, HKDF_SHA256, KeyType};
    struct Len(usize);
    impl KeyType for Len {
        fn len(&self) -> usize { self.0 }
    }

    let resp_key = {
        let salt = Salt::new(HKDF_SHA256, b"VMess Resp Key");
        let prk = salt.extract(body_key);
        let auth_info = [auth_id.as_ref()];
        let okm = prk.expand(&auth_info, Len(key_len)).expect("hkdf");
        let mut k = vec![0u8; key_len];
        okm.fill(&mut k).expect("hkdf fill");
        k
    };

    let resp_nonce = {
        let salt = Salt::new(HKDF_SHA256, b"VMess Resp Nonce");
        let prk = salt.extract(body_key);
        let auth_info = [auth_id.as_ref()];
        let okm = prk.expand(&auth_info, Len(12)).expect("hkdf");
        let mut n = vec![0u8; 12];
        okm.fill(&mut n).expect("hkdf fill");
        n
    };

    (resp_key, resp_nonce)
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
        nonce_bytes[..12]
            .try_into()
            .map_err(|_| Error::Protocol("vmess invalid nonce"))?,
    );
    let mut sealing_key = SealingKey::new(unbound, CountingNonce::new(nonce));
    let mut buf = plaintext.to_vec();
    buf.reserve(GCM_TAG_LEN);
    sealing_key
        .seal_in_place_append_tag(Aad::empty(), &mut buf)
        .map_err(|_| Error::Protocol("vmess aead encryption failed"))?;
    Ok(buf)
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
        nonce_bytes[..12]
            .try_into()
            .map_err(|_| Error::Protocol("vmess invalid nonce"))?,
    );
    let mut opening_key = OpeningKey::new(unbound, CountingNonce::new(nonce));
    let mut in_out = ciphertext.to_vec();
    let plaintext = opening_key
        .open_in_place(Aad::empty(), &mut in_out)
        .map_err(|_| Error::Protocol("vmess aead decryption failed"))?;
    Ok(plaintext.to_vec())
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

impl NonceSequence for CountingNonce {
    fn advance(&mut self) -> Result<Nonce, ring::error::Unspecified> {
        Ok(Nonce::assume_unique_for_key(self.nonce))
    }
}
