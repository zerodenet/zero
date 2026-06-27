// Mieru protocol inbound handler — inbound.rs

use alloc::vec::Vec;

use zero_core::{Error, ProtocolType};
use zero_traits::AsyncSocket;

use crate::crypto::{try_derive_keys, MieruCipher};
use crate::metadata::{
    DataMetadata, SessionMetadata, DATA_SERVER_TO_CLIENT, METADATA_LEN, OPEN_SESSION_REQUEST,
    OPEN_SESSION_RESPONSE,
};
use crate::segment::{build_data_segment, build_session_segment, parse_segment, Segment};
use crate::session::MieruSession;

/// Mieru inbound handler.
#[derive(Debug, Default, Clone)]
pub struct MieruInbound;

/// Result of accepting a mieru TCP connection.
///
/// The mieru session is target-agnostic: it is an encrypted tunnel. The proxy
/// target is conveyed by a socks5 request that the client sends inside the
/// tunnel after the handshake, so the caller must read that request over the
/// decrypted stream to obtain the target (mirroring the upstream mieru model).
pub struct MieruAccept {
    mieru_session: MieruSession,
    client_cipher: MieruCipher,
    server_cipher: MieruCipher,
    /// Bytes already decrypted from the first segment beyond its metadata
    /// (usually empty for socks5-in-tunnel clients).
    remaining_payload: Vec<u8>,
}

fn segment_wire_len(segment: &Segment, has_nonce: bool) -> usize {
    let nonce_len = if has_nonce { 24 } else { 0 };
    let meta_len = METADATA_LEN + 16;
    if let Some(meta) = segment.data_meta.as_ref() {
        nonce_len
            + meta_len
            + meta.prefix_length as usize
            + meta.payload_length as usize
            + if meta.payload_length > 0 { 16 } else { 0 }
            + meta.suffix_length as usize
    } else if let Some(meta) = segment.session_meta.as_ref() {
        nonce_len
            + meta_len
            + meta.payload_length as usize
            + if meta.payload_length > 0 { 16 } else { 0 }
    } else {
        nonce_len + meta_len
    }
}

/// Mieru inbound data-phase codec.
///
/// This type owns the protocol state for decrypting client-to-server data and
/// encrypting server-to-client data after the inbound handshake. Runtime code
/// should wrap it in an I/O adapter instead of touching ciphers or segment
/// metadata directly.
pub struct MieruInboundDataCodec {
    mieru_session: MieruSession,
    client_cipher: MieruCipher,
    server_cipher: MieruCipher,
    c2s_nonce_recv: bool,
    s2c_nonce_sent: bool,
}

impl MieruInboundDataCodec {
    pub fn new(accept: MieruAccept) -> (Self, Vec<u8>) {
        (
            Self {
                mieru_session: accept.mieru_session,
                client_cipher: accept.client_cipher,
                server_cipher: accept.server_cipher,
                c2s_nonce_recv: true,
                s2c_nonce_sent: true,
            },
            accept.remaining_payload,
        )
    }

    pub fn decrypt_client_data_with_consumed(
        &mut self,
        data: &[u8],
    ) -> Result<(Segment, usize), Error> {
        let include_nonce = !self.c2s_nonce_recv;
        let mut client_cipher = self.client_cipher.clone();
        let (segment, consumed) = parse_segment(data, &mut client_cipher, include_nonce, false)?;
        self.client_cipher = client_cipher;
        self.c2s_nonce_recv = true;
        let consumed = consumed.max(segment_wire_len(&segment, include_nonce));
        Ok((segment, consumed))
    }

    pub fn encrypt_server_data(&mut self, data: &[u8]) -> Result<Vec<u8>, Error> {
        let metadata = DataMetadata {
            protocol_type: DATA_SERVER_TO_CLIENT,
            timestamp: MieruSession::timestamp_minutes(),
            session_id: self.mieru_session.session_id,
            sequence_number: self.mieru_session.next_send_seq(),
            unack_sequence: 0,
            window_size: 1024,
            fragment_number: 0,
            prefix_length: 0,
            payload_length: data.len() as u16,
            suffix_length: 0,
        };
        let include_nonce = !self.s2c_nonce_sent;
        let segment = build_data_segment(&metadata, data, &mut self.server_cipher, include_nonce)?;
        self.s2c_nonce_sent = true;
        Ok(segment)
    }
}

impl MieruInbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Mieru
    }

    pub fn udp_session(&self) -> crate::udp::MieruInboundUdpSession {
        crate::udp::MieruInboundUdpSession::new()
    }

    /// Accept a mieru TCP connection — perform the mieru handshake only.
    ///
    /// Establishes the encrypted session and replies with openSessionResponse.
    /// The proxy target is NOT known here; the caller reads a socks5 request
    /// over the decrypted stream to obtain it.
    pub async fn accept_request<S: AsyncSocket>(
        &self,
        stream: &mut S,
        users: &[(String, String)],
    ) -> Result<MieruAccept, Error> {
        // Read first segment: nonce(24) + encrypted_meta(32) + tag(16) = 72 bytes.
        // Upstream mieru (and Zero's outbound) emit no leading padding0, so the
        // nonce is at offset 0.
        const SEGMENT_CORE: usize = 24 + 32 + 16;
        let mut first = vec![0u8; SEGMENT_CORE];
        read_exact(stream, &mut first, SEGMENT_CORE).await?;

        let unix_now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| Error::Protocol("mieru: system time error"))?
            .as_secs();

        // Try each user's key to decrypt the openSessionRequest metadata.
        let mut matched: Option<(MieruCipher, MieruCipher, SessionMetadata)> = None;

        for (username, password) in users {
            let keys = try_derive_keys(username, password, unix_now);
            for key in &keys {
                let mut c = MieruCipher::new(key);
                if let Ok(pt) = c.decrypt(true, &first) {
                    if pt.len() >= METADATA_LEN {
                        let meta = SessionMetadata::decode(&pt[..METADATA_LEN]);
                        if meta.protocol_type == OPEN_SESSION_REQUEST {
                            matched = Some((c, MieruCipher::new(key), meta));
                            break;
                        }
                    }
                }
            }
            if matched.is_some() {
                break;
            }
        }

        let (mut client_cipher, mut server_cipher, open_req) =
            matched.ok_or(Error::Protocol("mieru: no valid user key found"))?;

        // socks5-in-tunnel clients send no target in openSessionRequest. Consume
        // any declared payload defensively; the target arrives via a socks5
        // request in the data phase, read by the proxy handler.
        let remaining_payload = if open_req.payload_length > 0 {
            let plen = open_req.payload_length as usize;
            let mut payload_ct = vec![0u8; plen + 16]; // ciphertext + tag
            read_exact(stream, &mut payload_ct, plen + 16).await?;
            client_cipher.decrypt(false, &payload_ct)?
        } else {
            Vec::new()
        };

        // Send openSessionResponse.
        let session = MieruSession::with_id(open_req.session_id);
        let resp_meta = SessionMetadata {
            protocol_type: OPEN_SESSION_RESPONSE,
            timestamp: MieruSession::timestamp_minutes(),
            session_id: open_req.session_id,
            sequence_number: 0,
            status_code: 0,
            payload_length: 0,
            suffix_length: 0,
        };
        let resp_seg = build_session_segment(&resp_meta, &[], &mut server_cipher, true)?;
        stream
            .write_all(&resp_seg)
            .await
            .map_err(|_| Error::Io("mieru: write response"))?;

        Ok(MieruAccept {
            mieru_session: session,
            client_cipher,
            server_cipher,
            remaining_payload,
        })
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

async fn read_exact<S: AsyncSocket>(
    stream: &mut S,
    buf: &mut [u8],
    len: usize,
) -> Result<(), Error> {
    let mut offset = 0;
    while offset < len {
        let n = stream
            .read(&mut buf[offset..len])
            .await
            .map_err(|_| Error::Io("mieru read"))?;
        if n == 0 {
            return Err(Error::Protocol("mieru: connection closed"));
        }
        offset += n;
    }
    Ok(())
}
