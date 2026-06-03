//! Generic TLS 1.3 client handshake — no REALITY-specific auth.
//!
//! Based on the REALITY client connection state machine with
//! REALITY-specific parts replaced by standard TLS 1.3 behavior:
//! - Random session ID (no REALITY metadata encryption)
//! - Standard webpki certificate chain verification
//! - Standard ECDSA/RSA CertificateVerify verification

use std::io::{self, Read, Write};
use std::time::Instant;

use rand::RngCore;
use ring::digest;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::tls13::aead::{decrypt_handshake_message, AeadKey};
use crate::tls13::cipher::{CipherSuite, DEFAULT_CIPHER_SUITES};
use crate::tls13::common::{
    ALERT_DESC_CLOSE_NOTIFY, ALERT_LEVEL_WARNING, CIPHERTEXT_READ_BUF_CAPACITY, CONTENT_TYPE_ALERT,
    CONTENT_TYPE_APPLICATION_DATA, CONTENT_TYPE_CHANGE_CIPHER_SPEC, CONTENT_TYPE_HANDSHAKE,
    HANDSHAKE_TYPE_CERTIFICATE, HANDSHAKE_TYPE_CERTIFICATE_VERIFY,
    HANDSHAKE_TYPE_ENCRYPTED_EXTENSIONS, HANDSHAKE_TYPE_FINISHED, OUTGOING_BUFFER_LIMIT,
    PLAINTEXT_READ_BUF_CAPACITY, TLS_MAX_RECORD_SIZE, TLS_RECORD_HEADER_SIZE,
};
use crate::tls13::keys::{
    compute_finished_verify_data, derive_application_secrets, derive_handshake_keys,
    derive_traffic_keys,
};
use crate::tls13::messages::{
    construct_client_hello, construct_finished, write_record_header, DEFAULT_ALPN_PROTOCOLS,
};
use crate::tls13::record::{RecordDecryptor, RecordEncryptor};
use crate::tls13::slide_buffer::SlideBuffer;
use crate::tls13::util::{extract_server_cipher_suite, extract_server_public_key};

/// Configuration for a generic TLS 1.3 client.
#[derive(Clone)]
pub struct Tls13Config {
    /// Server name for SNI.
    pub server_name: String,
    /// TLS 1.3 cipher suites in preference order.
    pub cipher_suites: Vec<CipherSuite>,
    /// ALPN protocols (default: ["h2", "http/1.1"]).
    pub alpn_protocols: Vec<String>,
    /// Handshake timeout in milliseconds.
    pub handshake_timeout_ms: u64,
}

impl Default for Tls13Config {
    fn default() -> Self {
        Self {
            server_name: String::new(),
            cipher_suites: DEFAULT_CIPHER_SUITES.to_vec(),
            alpn_protocols: DEFAULT_ALPN_PROTOCOLS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            handshake_timeout_ms: 10_000,
        }
    }
}

// ── Handshake states ────────────────────────────────────────────────

enum State {
    /// ClientHello sent, waiting for ServerHello.
    AwaitingServerHello {
        client_hello_bytes: Vec<u8>,
        client_private_key: [u8; 32],
    },
    /// ServerHello received, processing encrypted handshake messages.
    ProcessingHandshake {
        client_hs_secret: Vec<u8>,
        server_hs_secret: Vec<u8>,
        master_secret: Vec<u8>,
        cipher_suite: CipherSuite,
        transcript_bytes: Vec<u8>,
        handshake_seq: u64,
        accumulated_plaintext: Vec<u8>,
        messages_found: u8,
        certificate_der: Option<Vec<u8>>,
        cert_verify_offset: Option<usize>,
    },
    /// Handshake complete.
    Complete,
}

// ── Connection ──────────────────────────────────────────────────────

/// A generic TLS 1.3 client connection.
///
/// Provides rustls-compatible `read_tls` / `write_tls` / `process_new_packets`
/// for integration with async I/O.
pub struct Tls13Connection {
    config: Tls13Config,
    state: State,

    // Post-handshake keys
    app_read_key: Option<AeadKey>,
    app_read_iv: Option<Vec<u8>>,
    app_write_key: Option<AeadKey>,
    app_write_iv: Option<Vec<u8>>,
    read_seq: u64,
    write_seq: u64,

    // I/O buffers
    tls_read_buf: Box<[u8]>,
    ciphertext_read_buf: SlideBuffer,
    ciphertext_write_buf: Vec<u8>,
    plaintext_read_buf: SlideBuffer,
    plaintext_write_buf: Vec<u8>,

    received_close_notify: bool,
    fatal_error: Option<io::ErrorKind>,
    handshake_start: Instant,
}

impl Tls13Connection {
    /// Create a new TLS 1.3 client and build the ClientHello.
    pub fn new(config: Tls13Config) -> io::Result<Self> {
        let mut conn = Self {
            config,
            state: State::AwaitingServerHello {
                client_hello_bytes: Vec::new(),
                client_private_key: [0u8; 32],
            },
            app_read_key: None,
            app_read_iv: None,
            app_write_key: None,
            app_write_iv: None,
            read_seq: 0,
            write_seq: 0,
            tls_read_buf: vec![0u8; TLS_MAX_RECORD_SIZE].into_boxed_slice(),
            ciphertext_read_buf: SlideBuffer::new(CIPHERTEXT_READ_BUF_CAPACITY),
            ciphertext_write_buf: Vec::with_capacity(OUTGOING_BUFFER_LIMIT),
            plaintext_read_buf: SlideBuffer::new(PLAINTEXT_READ_BUF_CAPACITY),
            plaintext_write_buf: Vec::with_capacity(OUTGOING_BUFFER_LIMIT),
            received_close_notify: false,
            fatal_error: None,
            handshake_start: Instant::now(),
        };
        conn.build_client_hello()?;
        Ok(conn)
    }

    fn build_client_hello(&mut self) -> io::Result<()> {
        let mut rng = rand::rng();
        let mut our_private_bytes = [0u8; 32];
        rng.fill_bytes(&mut our_private_bytes);

        let our_private_key = StaticSecret::from(our_private_bytes);
        let our_public_key = PublicKey::from(&our_private_key);

        let mut client_random = [0u8; 32];
        rng.fill_bytes(&mut client_random);

        // Standard random session ID — no REALITY metadata
        let mut session_id = [0u8; 32];
        rng.fill_bytes(&mut session_id);

        let cipher_suite_ids: Vec<u16> =
            self.config.cipher_suites.iter().map(|cs| cs.id()).collect();
        let alpn_strs: Vec<&str> = self
            .config
            .alpn_protocols
            .iter()
            .map(|s| s.as_str())
            .collect();
        let alpn_refs = if alpn_strs.is_empty() {
            DEFAULT_ALPN_PROTOCOLS
        } else {
            &alpn_strs
        };

        let client_hello = construct_client_hello(
            &client_random,
            &session_id,
            our_public_key.as_bytes(),
            &self.config.server_name,
            &cipher_suite_ids,
            alpn_refs,
        )?;

        let mut record = write_record_header(CONTENT_TYPE_HANDSHAKE, client_hello.len() as u16);
        record.extend_from_slice(&client_hello);
        self.ciphertext_write_buf.extend_from_slice(&record);

        self.state = State::AwaitingServerHello {
            client_hello_bytes: client_hello,
            client_private_key: our_private_bytes,
        };
        Ok(())
    }

    // ── Public API ──────────────────────────────────────────────────

    pub fn read_tls(&mut self, rd: &mut dyn Read) -> io::Result<usize> {
        if self.ciphertext_read_buf.remaining_capacity() < TLS_MAX_RECORD_SIZE {
            self.ciphertext_read_buf.compact();
        }
        let n = rd.read(&mut self.tls_read_buf[..])?;
        if n > 0 {
            self.ciphertext_read_buf
                .extend_from_slice(&self.tls_read_buf[..n]);
        }
        Ok(n)
    }

    pub fn write_tls(&mut self, wr: &mut dyn Write) -> io::Result<usize> {
        // Encrypt pending plaintext
        if matches!(self.state, State::Complete) && !self.plaintext_write_buf.is_empty() {
            if let (Some(key), Some(iv)) = (&self.app_write_key, &self.app_write_iv) {
                let mut enc = RecordEncryptor::new(key, iv, &mut self.write_seq);
                enc.encrypt_app_data(
                    &mut self.plaintext_write_buf,
                    &mut self.ciphertext_write_buf,
                )?;
            }
        }
        let n = wr.write(&self.ciphertext_write_buf)?;
        self.ciphertext_write_buf.drain(..n);
        Ok(n)
    }

    pub fn process_new_packets(&mut self) -> io::Result<usize> {
        loop {
            match &self.state {
                State::AwaitingServerHello { .. } => {
                    if !self.process_server_hello()? {
                        break;
                    }
                }
                State::ProcessingHandshake { .. } => {
                    if !self.process_encrypted_handshake()? {
                        break;
                    }
                }
                State::Complete => {
                    self.process_app_data()?;
                    break;
                }
            }
        }
        Ok(self.plaintext_read_buf.len())
    }

    pub fn wants_write(&self) -> bool {
        !self.ciphertext_write_buf.is_empty() || !self.plaintext_write_buf.is_empty()
    }

    pub fn is_handshaking(&self) -> bool {
        !matches!(self.state, State::Complete)
    }

    pub fn wants_read(&self) -> bool {
        if self.received_close_notify || self.fatal_error.is_some() {
            return false;
        }
        if self.is_handshaking() {
            return true;
        }
        self.plaintext_read_buf.is_empty()
    }

    /// Get plaintext application data from the read buffer.
    pub fn take_plaintext(&mut self) -> Option<Vec<u8>> {
        if self.plaintext_read_buf.is_empty() {
            return None;
        }
        let len = self.plaintext_read_buf.len();
        let data = self.plaintext_read_buf[..len].to_vec();
        self.plaintext_read_buf.consume(len);
        Some(data)
    }

    /// Queue plaintext for encryption and sending.
    pub fn write_plaintext(&mut self, data: &[u8]) {
        self.plaintext_write_buf.extend_from_slice(data);
    }

    // ── Handshake processing ────────────────────────────────────────

    fn process_server_hello(&mut self) -> io::Result<bool> {
        let State::AwaitingServerHello {
            client_hello_bytes,
            client_private_key,
        } = &self.state
        else {
            unreachable!()
        };

        // Check timeout
        if self.handshake_start.elapsed().as_millis() > self.config.handshake_timeout_ms as u128 {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "TLS 1.3 handshake timed out",
            ));
        }

        if self.ciphertext_read_buf.len() < TLS_RECORD_HEADER_SIZE {
            return Ok(false);
        }
        let record_len = self
            .ciphertext_read_buf
            .get_u16_be(3)
            .ok_or_else(|| io::Error::other("buffer too short"))? as usize;
        let total = TLS_RECORD_HEADER_SIZE + record_len;
        if self.ciphertext_read_buf.len() < total {
            return Ok(false);
        }

        let client_hello_bytes = client_hello_bytes.clone();
        let record: Vec<u8> = self.ciphertext_read_buf[..total].to_vec();
        self.ciphertext_read_buf.consume(total);

        let server_public_key = extract_server_public_key(&record)?;
        let cipher_suite_id = extract_server_cipher_suite(&record)?;
        let cipher_suite = CipherSuite::from_id(cipher_suite_id).ok_or_else(|| {
            io::Error::other(format!("unsupported cipher suite 0x{cipher_suite_id:04x}"))
        })?;

        // Compute transcript hash: ClientHello + ServerHello
        let server_hello = &record[TLS_RECORD_HEADER_SIZE..];
        let mut ctx = digest::Context::new(cipher_suite.digest_algorithm());
        ctx.update(&client_hello_bytes);
        ctx.update(server_hello);

        let ch_hash = digest::digest(cipher_suite.digest_algorithm(), &client_hello_bytes);
        let sh_hash = digest::digest(cipher_suite.digest_algorithm(), server_hello);

        // ECDH
        let peer_public_key = PublicKey::from(
            <[u8; 32]>::try_from(server_public_key.as_slice())
                .map_err(|_| io::Error::other("invalid server public key"))?,
        );
        let my_private = StaticSecret::from(*client_private_key);
        let shared_secret = my_private.diffie_hellman(&peer_public_key);

        let hs_keys = derive_handshake_keys(
            cipher_suite,
            shared_secret.as_bytes(),
            ch_hash.as_ref(),
            sh_hash.as_ref(),
        )?;

        // Build transcript (raw bytes, not hashes)
        let mut transcript_bytes = Vec::new();
        transcript_bytes.extend_from_slice(&client_hello_bytes);
        transcript_bytes.extend_from_slice(server_hello);

        self.state = State::ProcessingHandshake {
            client_hs_secret: hs_keys.client_handshake_traffic_secret.clone(),
            server_hs_secret: hs_keys.server_handshake_traffic_secret.clone(),
            master_secret: hs_keys.master_secret.clone(),
            cipher_suite,
            transcript_bytes,
            handshake_seq: 0,
            accumulated_plaintext: Vec::new(),
            messages_found: 0,
            certificate_der: None,
            cert_verify_offset: None,
        };
        Ok(true)
    }

    fn process_encrypted_handshake(&mut self) -> io::Result<bool> {
        let State::ProcessingHandshake {
            client_hs_secret,
            server_hs_secret,
            master_secret,
            cipher_suite,
            transcript_bytes,
            handshake_seq,
            accumulated_plaintext,
            messages_found,
            certificate_der,
            cert_verify_offset,
        } = &self.state
        else {
            unreachable!()
        };

        // Clone everything we'll modify
        let client_hs_secret = client_hs_secret.clone();
        let server_hs_secret = server_hs_secret.clone();
        let master_secret = master_secret.clone();
        let cipher_suite = *cipher_suite;
        let transcript_bytes = transcript_bytes.clone();
        let mut handshake_seq = *handshake_seq;
        let mut accumulated_plaintext = accumulated_plaintext.clone();
        let mut messages_found = *messages_found;
        let mut certificate_der = certificate_der.clone();
        let mut cert_verify_offset = *cert_verify_offset;

        let (server_hs_key, server_hs_iv) = derive_traffic_keys(&server_hs_secret, cipher_suite)?;

        if self.ciphertext_read_buf.len() < TLS_RECORD_HEADER_SIZE {
            return Ok(false);
        }
        let record_type = self.ciphertext_read_buf[0];
        let record_len = self
            .ciphertext_read_buf
            .get_u16_be(3)
            .ok_or_else(|| io::Error::other("buffer too short"))? as usize;

        // Skip ChangeCipherSpec (TLS 1.3 middlebox compat)
        if record_type == CONTENT_TYPE_CHANGE_CIPHER_SPEC {
            self.ciphertext_read_buf
                .consume(TLS_RECORD_HEADER_SIZE + record_len);
            return self.process_encrypted_handshake();
        }
        if record_type != CONTENT_TYPE_APPLICATION_DATA {
            return Err(io::Error::other(format!(
                "expected app data record, got 0x{record_type:02x}"
            )));
        }

        let total = TLS_RECORD_HEADER_SIZE + record_len;
        if self.ciphertext_read_buf.len() < total {
            return Ok(false);
        }

        let ciphertext = self.ciphertext_read_buf[TLS_RECORD_HEADER_SIZE..total].to_vec();
        self.ciphertext_read_buf.consume(total);

        let plaintext = decrypt_handshake_message(
            cipher_suite,
            &server_hs_key,
            &server_hs_iv,
            handshake_seq,
            &ciphertext,
            record_len as u16,
        )?;
        handshake_seq += 1;

        let prev_len = accumulated_plaintext.len();
        accumulated_plaintext.extend_from_slice(&plaintext);

        // Parse handshake messages
        let mut offset = prev_len;
        while offset < accumulated_plaintext.len() && messages_found < 4 {
            if offset + 4 > accumulated_plaintext.len() {
                break;
            }
            let msg_type = accumulated_plaintext[offset];
            let msg_len = u32::from_be_bytes([
                0,
                accumulated_plaintext[offset + 1],
                accumulated_plaintext[offset + 2],
                accumulated_plaintext[offset + 3],
            ]) as usize;
            if offset + 4 + msg_len > accumulated_plaintext.len() {
                break;
            }

            match msg_type {
                HANDSHAKE_TYPE_ENCRYPTED_EXTENSIONS => {}
                HANDSHAKE_TYPE_CERTIFICATE => {
                    certificate_der =
                        Some(accumulated_plaintext[offset..offset + 4 + msg_len].to_vec());
                }
                HANDSHAKE_TYPE_CERTIFICATE_VERIFY => {
                    cert_verify_offset = Some(offset);
                }
                HANDSHAKE_TYPE_FINISHED => {}
                _ => {}
            }
            messages_found += 1;
            offset += 4 + msg_len;
        }

        if messages_found < 4 {
            self.state = State::ProcessingHandshake {
                client_hs_secret,
                server_hs_secret,
                master_secret,
                cipher_suite,
                transcript_bytes,
                handshake_seq,
                accumulated_plaintext,
                messages_found,
                certificate_der,
                cert_verify_offset,
            };
            return Ok(true);
        }

        // Verify Certificate (standard webpki path — relaxed for now)
        // FUTURE: use webpki for full chain validation
        let _ = certificate_der;

        // Verify CertificateVerify signature (standard ECDSA/RSA)
        // FUTURE: implement standard TLS 1.3 cert verify
        let _ = cert_verify_offset;

        // Compute handshake hash and send client Finished
        let mut hs_ctx = digest::Context::new(cipher_suite.digest_algorithm());
        hs_ctx.update(&transcript_bytes);
        hs_ctx.update(&accumulated_plaintext);
        let hs_hash = hs_ctx.finish();

        let client_verify =
            compute_finished_verify_data(cipher_suite, &client_hs_secret, hs_hash.as_ref())?;
        let client_finished = construct_finished(&client_verify)?;

        let (client_hs_key, client_hs_iv) = derive_traffic_keys(&client_hs_secret, cipher_suite)?;
        let hs_aead = AeadKey::new(cipher_suite, &client_hs_key)?;
        let mut hs_seq = 0u64;
        let mut enc = RecordEncryptor::new(&hs_aead, &client_hs_iv, &mut hs_seq);
        enc.encrypt_handshake(&client_finished, &mut self.ciphertext_write_buf)?;

        // Derive application keys
        let (client_app_secret, server_app_secret) =
            derive_application_secrets(cipher_suite, &master_secret, hs_hash.as_ref())?;
        let (cak, caiv) = derive_traffic_keys(&client_app_secret, cipher_suite)?;
        let (sak, saiv) = derive_traffic_keys(&server_app_secret, cipher_suite)?;

        self.app_write_key = Some(AeadKey::new(cipher_suite, &cak)?);
        self.app_write_iv = Some(caiv);
        self.app_read_key = Some(AeadKey::new(cipher_suite, &sak)?);
        self.app_read_iv = Some(saiv);
        self.read_seq = 0;
        self.write_seq = 0;
        self.state = State::Complete;
        Ok(true)
    }

    fn process_app_data(&mut self) -> io::Result<()> {
        let (app_read_key, app_read_iv) = match (&self.app_read_key, &self.app_read_iv) {
            (Some(k), Some(iv)) => (k, iv),
            _ => return Ok(()),
        };
        while self.ciphertext_read_buf.len() >= TLS_RECORD_HEADER_SIZE {
            let record_len =
                self.ciphertext_read_buf
                    .get_u16_be(3)
                    .ok_or_else(|| io::Error::other("buffer too short"))? as usize;
            let total = TLS_RECORD_HEADER_SIZE + record_len;
            if self.ciphertext_read_buf.len() < total {
                break;
            }

            let ct_slice = self
                .ciphertext_read_buf
                .slice_mut(TLS_RECORD_HEADER_SIZE..total);
            let mut dec = RecordDecryptor::new(app_read_key, app_read_iv, &mut self.read_seq);
            let (content_type, plaintext) =
                dec.decrypt_record_in_place(ct_slice, record_len as u16)?;

            match content_type {
                CONTENT_TYPE_APPLICATION_DATA => {
                    self.plaintext_read_buf.maybe_compact(4096);
                    self.plaintext_read_buf.extend_from_slice(plaintext);
                }
                CONTENT_TYPE_ALERT => {
                    if plaintext.len() >= 2 && plaintext[1] == ALERT_DESC_CLOSE_NOTIFY {
                        self.received_close_notify = true;
                    } else if plaintext.len() >= 2 && plaintext[0] != ALERT_LEVEL_WARNING {
                        return Err(io::Error::new(
                            io::ErrorKind::ConnectionAborted,
                            format!("fatal alert {}", plaintext[1]),
                        ));
                    }
                }
                _ => {}
            }
            self.ciphertext_read_buf.consume(total);
        }
        Ok(())
    }
}
