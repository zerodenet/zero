//! Shared cryptographic primitives for VMess AEAD protocol.
//!
//! This module centralises all key-derivation, AEAD seal/open, and nonce
//! management used by both the inbound and outbound handlers. It also provides
//! the body-level AEAD framing needed for encrypted data relay after the
//! handshake phase.

use ring::aead::{Aad, BoundKey, Nonce, NonceSequence, OpeningKey, SealingKey, UnboundKey};
use ring::hkdf::{KeyType, Salt, HKDF_SHA256};
use ring::hmac;

use zero_core::Error;

use crate::shared::VmessCipher;

// ── Constants ────────────────────────────────────────────────────────

pub(crate) const GCM_TAG_LEN: usize = 16;
pub(crate) const NONCE_LEN: usize = 12;

/// Number of chunks before a re-key cycle (VMess AEAD spec).
const REKEY_INTERVAL: u64 = 1 << 14; // 16384

// ── HKDF key-length helper ──────────────────────────────────────────

struct HkdfLen(usize);

impl KeyType for HkdfLen {
    fn len(&self) -> usize {
        self.0
    }
}

// ── Header crypto ───────────────────────────────────────────────────

/// Derive the per-user command key from a VMess UUID.
pub(crate) fn derive_cmd_key(uuid: &[u8; 16], key_len: usize) -> Vec<u8> {
    let salt = Salt::new(HKDF_SHA256, b"VMess AEAD KDF");
    let prk = salt.extract(uuid);
    let info: [&[u8]; 1] = [b""];
    let okm = prk
        .expand(&info, HkdfLen(key_len))
        .expect("hkdf expand cmd_key");
    let mut key = vec![0u8; key_len];
    okm.fill(&mut key).expect("hkdf fill cmd_key");
    key
}

/// Compute the 16-byte auth ID = HMAC-SHA256(cmd_key, timestamp)[:16].
pub(crate) fn compute_auth_id(cmd_key: &[u8], timestamp: u64) -> [u8; 16] {
    let key = hmac::Key::new(hmac::HMAC_SHA256, cmd_key);
    let tag = hmac::sign(&key, &timestamp.to_be_bytes());
    let mut result = [0u8; 16];
    result.copy_from_slice(&tag.as_ref()[..16]);
    result
}

/// Derive body key and nonce from the command key and auth ID.
pub(crate) fn derive_body_key_nonce(
    cmd_key: &[u8],
    auth_id: &[u8; 16],
    key_len: usize,
) -> (Vec<u8>, Vec<u8>) {
    let body_key = hkdf_expand(cmd_key, b"VMess Body Key", auth_id, key_len);
    let body_nonce = hkdf_expand(cmd_key, b"VMess Body Nonce", auth_id, NONCE_LEN);
    (body_key, body_nonce)
}

/// Derive response key and nonce from the body key and auth ID.
pub(crate) fn derive_response_key_nonce(
    body_key: &[u8],
    auth_id: &[u8; 16],
    key_len: usize,
) -> (Vec<u8>, Vec<u8>) {
    let resp_key = hkdf_expand(body_key, b"VMess Resp Key", auth_id, key_len);
    let resp_nonce = hkdf_expand(body_key, b"VMess Resp Nonce", auth_id, NONCE_LEN);
    (resp_key, resp_nonce)
}

// ── AEAD seal / open (header) ──────────────────────────────────────

pub(crate) fn aead_encrypt(
    key: &[u8],
    nonce_bytes: &[u8],
    plaintext: &[u8],
    cipher: VmessCipher,
) -> Result<Vec<u8>, Error> {
    let unbound = UnboundKey::new(cipher.aead_algorithm(), key)
        .map_err(|_| Error::Protocol("vmess invalid aead key"))?;
    let nonce = Nonce::assume_unique_for_key(
        nonce_bytes[..NONCE_LEN]
            .try_into()
            .map_err(|_| Error::Protocol("vmess invalid nonce length"))?,
    );
    let mut sealing_key = SealingKey::new(unbound, CountingNonce::new(nonce));
    let mut buf = plaintext.to_vec();
    buf.reserve(GCM_TAG_LEN);
    sealing_key
        .seal_in_place_append_tag(Aad::empty(), &mut buf)
        .map_err(|_| Error::Protocol("vmess aead encryption failed"))?;
    Ok(buf)
}

pub(crate) fn aead_decrypt(
    key: &[u8],
    nonce_bytes: &[u8],
    ciphertext: &[u8],
    cipher: VmessCipher,
) -> Result<Vec<u8>, Error> {
    let unbound = UnboundKey::new(cipher.aead_algorithm(), key)
        .map_err(|_| Error::Protocol("vmess invalid aead key"))?;
    let nonce = Nonce::assume_unique_for_key(
        nonce_bytes[..NONCE_LEN]
            .try_into()
            .map_err(|_| Error::Protocol("vmess invalid nonce length"))?,
    );
    let mut opening_key = OpeningKey::new(unbound, CountingNonce::new(nonce));
    let mut in_out = ciphertext.to_vec();
    let plaintext = opening_key
        .open_in_place(Aad::empty(), &mut in_out)
        .map_err(|_| Error::Protocol("vmess aead decryption failed"))?;
    Ok(plaintext.to_vec())
}

// ── AEAD body framing ──────────────────────────────────────────────

/// VMess AEAD body chunk reader / writer state machine.
///
/// After the header handshake, data is relayed in AEAD-encrypted chunks:
///
/// ```text
/// [2-byte payload length (encrypted)][16-byte tag][encrypted payload][16-byte tag]
/// ```
///
/// The nonce increments by 2 per chunk (one for length, one for payload).
/// After `2^14` chunks the key is rotated via HKDF.
#[allow(dead_code)]
pub(crate) struct BodyAead {
    key: Vec<u8>,
    nonce_counter: u64,
    cipher: VmessCipher,
    chunks_since_rekey: u64,
}

impl BodyAead {
    pub fn new(key: Vec<u8>, nonce_prefix: &[u8], cipher: VmessCipher) -> Self {
        // nonce = nonce_prefix (12 bytes) XOR big-endian(counter)
        // For the initial nonce we start from the prefix as-is (counter=0 was
        // used by the header; body starts from the next nonce).
        let nonce_val = u64::from_be_bytes(nonce_prefix[4..12].try_into().unwrap());
        Self {
            key,
            nonce_counter: nonce_val.wrapping_add(1),
            cipher,
            chunks_since_rekey: 0,
        }
    }

    /// Encrypt a single payload chunk, returning the framed wire bytes:
    /// `[enc_len(2+tag)][enc_payload(len+tag)]`.
    pub fn seal_chunk(&mut self, payload: &[u8]) -> Result<Vec<u8>, Error> {
        let nonce_len = self.next_nonce();
        let nonce_payload = self.next_nonce();

        // Encrypt 2-byte length
        let len_bytes = (payload.len() as u16).to_be_bytes();
        let enc_len = aead_encrypt_with_nonce(&self.key, &nonce_len, &len_bytes, self.cipher)?;

        // Encrypt payload
        let enc_payload = aead_encrypt_with_nonce(&self.key, &nonce_payload, payload, self.cipher)?;

        let mut out = Vec::with_capacity(enc_len.len() + enc_payload.len());
        out.extend_from_slice(&enc_len);
        out.extend_from_slice(&enc_payload);

        self.chunks_since_rekey += 1;
        if self.chunks_since_rekey >= REKEY_INTERVAL {
            self.rekey();
        }

        Ok(out)
    }

    /// Decrypt a framed chunk. `buf` must contain the full framed data:
    /// `[enc_len(2+tag)][enc_payload(len+tag)]`.
    /// Returns the decrypted payload.
    pub fn open_chunk(&mut self, buf: &[u8]) -> Result<Vec<u8>, Error> {
        if buf.len() < 2 + GCM_TAG_LEN {
            return Err(Error::Protocol("vmess body chunk too short"));
        }

        let nonce_len = self.next_nonce();
        let nonce_payload = self.next_nonce();

        // Decrypt 2-byte length
        let len_plain = aead_decrypt_with_nonce(
            &self.key,
            &nonce_len,
            &buf[..2 + GCM_TAG_LEN],
            self.cipher,
        )?;
        let payload_len = u16::from_be_bytes(
            len_plain[..2]
                .try_into()
                .map_err(|_| Error::Protocol("vmess body chunk len decode"))?,
        ) as usize;

        let payload_start = 2 + GCM_TAG_LEN;
        let payload_end = payload_start + payload_len + GCM_TAG_LEN;
        if buf.len() < payload_end {
            return Err(Error::Protocol("vmess body chunk truncated"));
        }

        let payload_plain = aead_decrypt_with_nonce(
            &self.key,
            &nonce_payload,
            &buf[payload_start..payload_end],
            self.cipher,
        )?;

        self.chunks_since_rekey += 1;
        if self.chunks_since_rekey >= REKEY_INTERVAL {
            self.rekey();
        }

        Ok(payload_plain)
    }

    /// Length of the encrypted frame for a given plaintext payload length.
    /// `(2 + tag) + (payload_len + tag)`.
    pub fn framed_len(payload_len: usize) -> usize {
        (2 + GCM_TAG_LEN) + (payload_len + GCM_TAG_LEN)
    }

    // ── internals ──

    fn next_nonce(&mut self) -> [u8; NONCE_LEN] {
        let counter = self.nonce_counter;
        self.nonce_counter = self.nonce_counter.wrapping_add(1);
        let mut nonce = [0u8; NONCE_LEN];
        nonce[4..12].copy_from_slice(&counter.to_be_bytes());
        nonce
    }

    fn rekey(&mut self) {
        // new_key = HKDF(old_key, "VMess AEAD Body Key", info=counter)
        let counter_bytes = self.chunks_since_rekey.to_be_bytes();
        self.key = hkdf_expand(
            &self.key,
            b"VMess AEAD Body Key",
            &counter_bytes,
            self.cipher.key_len(),
        );
        self.chunks_since_rekey = 0;
    }
}

// ── AEAD helpers with explicit nonce (no counter) ──────────────────

fn aead_encrypt_with_nonce(
    key: &[u8],
    nonce: &[u8; NONCE_LEN],
    plaintext: &[u8],
    cipher: VmessCipher,
) -> Result<Vec<u8>, Error> {
    let unbound = UnboundKey::new(cipher.aead_algorithm(), key)
        .map_err(|_| Error::Protocol("vmess invalid aead key"))?;
    let ring_nonce = Nonce::assume_unique_for_key(*nonce);
    let mut sealing_key = SealingKey::new(unbound, SingleNonce::new(ring_nonce));
    let mut buf = plaintext.to_vec();
    buf.reserve(GCM_TAG_LEN);
    sealing_key
        .seal_in_place_append_tag(Aad::empty(), &mut buf)
        .map_err(|_| Error::Protocol("vmess body aead encryption failed"))?;
    Ok(buf)
}

fn aead_decrypt_with_nonce(
    key: &[u8],
    nonce: &[u8; NONCE_LEN],
    ciphertext: &[u8],
    cipher: VmessCipher,
) -> Result<Vec<u8>, Error> {
    let unbound = UnboundKey::new(cipher.aead_algorithm(), key)
        .map_err(|_| Error::Protocol("vmess invalid aead key"))?;
    let ring_nonce = Nonce::assume_unique_for_key(*nonce);
    let mut opening_key = OpeningKey::new(unbound, SingleNonce::new(ring_nonce));
    let mut in_out = ciphertext.to_vec();
    let plaintext = opening_key
        .open_in_place(Aad::empty(), &mut in_out)
        .map_err(|_| Error::Protocol("vmess body aead decryption failed"))?;
    Ok(plaintext.to_vec())
}

// ── Nonce sequence implementations ─────────────────────────────────

/// Nonce that returns the same value every time (for header AEAD).
pub(crate) struct CountingNonce {
    nonce: [u8; NONCE_LEN],
}

impl CountingNonce {
    pub fn new(initial: Nonce) -> Self {
        let mut nonce = [0u8; NONCE_LEN];
        nonce.copy_from_slice(initial.as_ref());
        Self { nonce }
    }
}

impl NonceSequence for CountingNonce {
    fn advance(&mut self) -> Result<Nonce, ring::error::Unspecified> {
        Ok(Nonce::assume_unique_for_key(self.nonce))
    }
}

/// Single-use nonce (for body AEAD per-chunk operations).
struct SingleNonce([u8; NONCE_LEN]);

impl SingleNonce {
    fn new(nonce: Nonce) -> Self {
        let mut buf = [0u8; NONCE_LEN];
        buf.copy_from_slice(nonce.as_ref());
        Self(buf)
    }
}

impl NonceSequence for SingleNonce {
    fn advance(&mut self) -> Result<Nonce, ring::error::Unspecified> {
        Ok(Nonce::assume_unique_for_key(self.0))
    }
}

// ── Helpers ────────────────────────────────────────────────────────

fn hkdf_expand(secret: &[u8], salt_label: &[u8], info: &[u8], len: usize) -> Vec<u8> {
    let salt = Salt::new(HKDF_SHA256, salt_label);
    let prk = salt.extract(secret);
    let info_slices = [info];
    let okm = prk.expand(&info_slices, HkdfLen(len)).expect("hkdf expand");
    let mut buf = vec![0u8; len];
    okm.fill(&mut buf).expect("hkdf fill");
    buf
}

pub(crate) fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── Hex helper (no external dep) ──────────────────────────────────

pub(crate) mod hex {
    pub fn encode(bytes: &[u8; 16]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
}
