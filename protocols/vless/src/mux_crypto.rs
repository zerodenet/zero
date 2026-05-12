// MUX stream-level encryption — mux_crypto.rs
//
// Each MUX stream gets a unique AES-128-GCM key derived from the master UUID
// + stream_id. Frame headers (session_id, length) stay plaintext for routing;
// only the payload is encrypted.
//
// Key:  HKDF-SHA256(master_uuid, salt=stream_id_be, info="vless mux stream key")
// Nonce: counter_be(8) || stream_id_be(2) || zeros(2)
//
// Enabled when flow is "xtls-rprx-vision" (mux_encryption flag).

use alloc::vec::Vec;

use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_128_GCM};
use ring::hkdf;

use zero_core::Error;

const AEAD_KEY_LEN: usize = 16;
const AEAD_NONCE_LEN: usize = 12;
const AEAD_TAG_LEN: usize = 16;

/// Manages per-stream encryption keys for a MUX session.
///
/// Keys are lazily derived from the master UUID when a new stream is first used.
pub struct MuxCrypto {
    master_key: [u8; AEAD_KEY_LEN],
    /// (client_to_server_key, server_to_client_key) per stream_id, plus counters
    streams: Vec<Option<StreamKeys>>,
}

struct StreamKeys {
    c2s: StreamCipher,
    s2c: StreamCipher,
}

struct StreamCipher {
    key: LessSafeKey,
    counter: u64,
}

impl MuxCrypto {
    /// Create a new MuxCrypto from the master UUID.
    pub fn new(master_uuid: &[u8; 16]) -> Self {
        // Derive master key: HKDF-Extract(salt=zeros, ikm=uuid) → then expand
        let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, &[]);
        let prk = salt.extract(master_uuid);
        let mut master_key = [0u8; AEAD_KEY_LEN];
        prk.expand(&[b"vless mux master key"], MuxKeyLen)
            .and_then(|okm| okm.fill(&mut master_key))
            .expect("HKDF expand for AES-128 key len is infallible");

        Self {
            master_key,
            streams: Vec::new(),
        }
    }

    /// Ensure stream keys exist for the given stream_id.
    fn ensure_stream_keys(&mut self, stream_id: u16) -> &mut StreamKeys {
        let idx = stream_id as usize;
        while self.streams.len() <= idx {
            self.streams.push(None);
        }
        if self.streams[idx].is_none() {
            self.streams[idx] = Some(StreamKeys {
                c2s: StreamCipher {
                    key: derive_stream_key(&self.master_key, stream_id, b"c2s"),
                    counter: 0,
                },
                s2c: StreamCipher {
                    key: derive_stream_key(&self.master_key, stream_id, b"s2c"),
                    counter: 0,
                },
            });
        }
        self.streams[idx].as_mut().unwrap()
    }

    /// Encrypt payload for client → server direction.
    /// Returns the encrypted payload (payload + 16-byte GCM tag).
    pub fn encrypt_c2s(&mut self, stream_id: u16, payload: &[u8]) -> Result<Vec<u8>, Error> {
        let keys = self.ensure_stream_keys(stream_id);
        let counter = keys.c2s.counter;
        keys.c2s.counter += 1;
        encrypt_internal(&keys.c2s.key, stream_id, counter, payload)
    }

    /// Decrypt payload from client → server direction.
    pub fn decrypt_c2s(&mut self, stream_id: u16, payload: &[u8]) -> Result<Vec<u8>, Error> {
        let keys = self.ensure_stream_keys(stream_id);
        let counter = keys.c2s.counter;
        keys.c2s.counter += 1;
        decrypt_internal(&keys.c2s.key, stream_id, counter, payload)
    }

    /// Encrypt payload for server → client direction.
    pub fn encrypt_s2c(&mut self, stream_id: u16, payload: &[u8]) -> Result<Vec<u8>, Error> {
        let keys = self.ensure_stream_keys(stream_id);
        let counter = keys.s2c.counter;
        keys.s2c.counter += 1;
        encrypt_internal(&keys.s2c.key, stream_id, counter, payload)
    }

    /// Decrypt payload from server → client direction.
    pub fn decrypt_s2c(&mut self, stream_id: u16, payload: &[u8]) -> Result<Vec<u8>, Error> {
        let keys = self.ensure_stream_keys(stream_id);
        let counter = keys.s2c.counter;
        keys.s2c.counter += 1;
        decrypt_internal(&keys.s2c.key, stream_id, counter, payload)
    }
}

fn derive_stream_key(master_key: &[u8; 16], stream_id: u16, direction: &[u8]) -> LessSafeKey {
    let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, &stream_id.to_be_bytes());
    let prk = salt.extract(master_key);
    let mut key_bytes = [0u8; AEAD_KEY_LEN];
    prk.expand(&[b"vless mux stream key", direction], MuxKeyLen)
        .and_then(|okm| okm.fill(&mut key_bytes))
        .expect("HKDF expand for AES-128 key len is infallible");
    let unbound = UnboundKey::new(&AES_128_GCM, &key_bytes).expect("AES_128_GCM key is valid");
    LessSafeKey::new(unbound)
}

fn build_nonce(stream_id: u16, counter: u64) -> [u8; AEAD_NONCE_LEN] {
    let mut nonce = [0u8; AEAD_NONCE_LEN];
    nonce[..8].copy_from_slice(&counter.to_be_bytes());
    nonce[8..10].copy_from_slice(&stream_id.to_be_bytes());
    // last 2 bytes are zero
    nonce
}

fn encrypt_internal(
    key: &LessSafeKey,
    stream_id: u16,
    counter: u64,
    plaintext: &[u8],
) -> Result<Vec<u8>, Error> {
    let nonce_bytes = build_nonce(stream_id, counter);
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);

    let mut buf = Vec::with_capacity(plaintext.len() + AEAD_TAG_LEN);
    buf.extend_from_slice(plaintext);
    key.seal_in_place_append_tag(nonce, Aad::empty(), &mut buf)
        .map_err(|_| Error::Protocol("MUX stream encryption failed"))?;
    Ok(buf)
}

fn decrypt_internal(
    key: &LessSafeKey,
    stream_id: u16,
    counter: u64,
    ciphertext: &[u8],
) -> Result<Vec<u8>, Error> {
    if ciphertext.len() < AEAD_TAG_LEN {
        return Err(Error::Protocol("MUX stream ciphertext too short"));
    }
    let nonce_bytes = build_nonce(stream_id, counter);
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);

    let mut buf = ciphertext.to_vec();
    let decrypted = key
        .open_in_place(nonce, Aad::empty(), &mut buf)
        .map_err(|_| Error::Protocol("MUX stream decryption failed"))?;
    Ok(decrypted.to_vec())
}

struct MuxKeyLen;

impl hkdf::KeyType for MuxKeyLen {
    fn len(&self) -> usize {
        AEAD_KEY_LEN
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::parse_uuid;

    #[test]
    fn test_key_derivation_determinism() {
        let uuid = parse_uuid("b831381d-6324-4d53-ad4f-8cda48b30811").unwrap();
        let mut crypto = MuxCrypto::new(&uuid);

        let plaintext = b"hello mux stream";
        let ct1 = crypto.encrypt_c2s(1, plaintext).unwrap();
        // Same stream, same counter for server decrypt
        let mut crypto2 = MuxCrypto::new(&uuid);
        let pt = crypto2.decrypt_c2s(1, &ct1).unwrap();
        assert_eq!(pt, plaintext);
    }

    #[test]
    fn test_roundtrip_both_directions() {
        let uuid = parse_uuid("b831381d-6324-4d53-ad4f-8cda48b30811").unwrap();
        let mut client = MuxCrypto::new(&uuid);
        let mut server = MuxCrypto::new(&uuid);

        let data = b"bidirectional test payload";
        let ct = client.encrypt_c2s(5, data).unwrap();
        let pt = server.decrypt_c2s(5, &ct).unwrap();
        assert_eq!(pt, data);

        let ct = server.encrypt_s2c(5, data).unwrap();
        let pt = client.decrypt_s2c(5, &ct).unwrap();
        assert_eq!(pt, data);
    }

    #[test]
    fn test_multiple_streams_independent() {
        let uuid = parse_uuid("b831381d-6324-4d53-ad4f-8cda48b30811").unwrap();
        let mut crypto = MuxCrypto::new(&uuid);

        let ct1 = crypto.encrypt_c2s(1, b"stream 1 data").unwrap();
        let ct2 = crypto.encrypt_c2s(2, b"stream 2 data").unwrap();

        let mut crypto2 = MuxCrypto::new(&uuid);
        let pt1 = crypto2.decrypt_c2s(1, &ct1).unwrap();
        let pt2 = crypto2.decrypt_c2s(2, &ct2).unwrap();

        assert_eq!(pt1, b"stream 1 data");
        assert_eq!(pt2, b"stream 2 data");
    }

    #[test]
    fn test_counter_increment() {
        let uuid = parse_uuid("b831381d-6324-4d53-ad4f-8cda48b30811").unwrap();
        let mut client = MuxCrypto::new(&uuid);

        let ct1 = client.encrypt_c2s(1, b"msg1").unwrap();
        let ct2 = client.encrypt_c2s(1, b"msg2").unwrap();
        assert_ne!(ct1, ct2); // different nonces produce different ciphertexts

        let mut server = MuxCrypto::new(&uuid);
        let pt1 = server.decrypt_c2s(1, &ct1).unwrap();
        let pt2 = server.decrypt_c2s(1, &ct2).unwrap();
        assert_eq!(pt1, b"msg1");
        assert_eq!(pt2, b"msg2");
    }

    #[test]
    fn test_wrong_key_fails() {
        let uuid1 = parse_uuid("b831381d-6324-4d53-ad4f-8cda48b30811").unwrap();
        let uuid2 = parse_uuid("a831381d-6324-4d53-ad4f-8cda48b30811").unwrap();
        let mut client = MuxCrypto::new(&uuid1);
        let mut server = MuxCrypto::new(&uuid2);

        let ct = client.encrypt_c2s(1, b"test").unwrap();
        assert!(server.decrypt_c2s(1, &ct).is_err());
    }

    #[test]
    fn test_empty_payload() {
        let uuid = parse_uuid("b831381d-6324-4d53-ad4f-8cda48b30811").unwrap();
        let mut client = MuxCrypto::new(&uuid);
        let mut server = MuxCrypto::new(&uuid);

        // Empty payload is valid (used as stream close notification)
        let ct = client.encrypt_c2s(1, b"").unwrap();
        // ciphertext is just the 16-byte GCM tag
        assert_eq!(ct.len(), 16);
        let pt = server.decrypt_c2s(1, &ct).unwrap();
        assert!(pt.is_empty());
    }
}
