// Mieru protocol outbound handler — outbound.rs

use alloc::vec::Vec;

use zero_core::{Address, Error};
use zero_traits::AsyncSocket;

use crate::crypto::{derive_key, MieruCipher, NonceConfig};
use crate::metadata::{
    DataMetadata, SessionMetadata, CLOSE_SESSION_REQUEST, DATA_CLIENT_TO_SERVER, METADATA_LEN,
    OPEN_SESSION_REQUEST, OPEN_SESSION_RESPONSE,
};
use crate::segment::{build_data_segment, build_session_segment, parse_segment, Segment};
use crate::session::MieruSession;

/// Mieru outbound connection.
pub struct MieruOutbound {
    pub mieru_session: MieruSession,
    pub client_cipher: MieruCipher,
    pub server_cipher: MieruCipher,
    pub c2s_nonce_sent: bool,
    pub s2c_nonce_recv: bool,
}

impl MieruOutbound {
    /// Perform the mieru outbound handshake.
    pub async fn connect<S: AsyncSocket>(
        stream: &mut S,
        username: &str,
        password: &str,
        target: &Address,
        port: u16,
    ) -> Result<Self, Error> {
        let unix_now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| Error::Protocol("mieru: time"))?
            .as_secs();

        let key = derive_key(username, password, unix_now);
        let nc = NonceConfig {
            username: Some(username.to_owned()),
            ..Default::default()
        };
        let mut client_cipher = MieruCipher::with_config(&key, &nc);
        let mut server_cipher = MieruCipher::with_config(&key, &nc);
        let session = MieruSession::new();

        // Encode target + send openSessionRequest
        let target_payload = encode_target(target, port);
        let open_meta = SessionMetadata {
            protocol_type: OPEN_SESSION_REQUEST,
            timestamp: MieruSession::timestamp_minutes(),
            session_id: session.session_id,
            sequence_number: 0,
            status_code: 0,
            payload_length: target_payload.len() as u16,
            suffix_length: 0,
        };
        let open_seg =
            build_session_segment(&open_meta, &target_payload, &mut client_cipher, true)?;
        stream
            .write_all(&open_seg)
            .await
            .map_err(|_| Error::Io("mieru: send open"))?;

        // Read openSessionResponse: padding0(0-64) + nonce(24) + meta(32) + tag(16) + optional payload
        // Read enough to cover max padding + core segment
        const MAX_PADDING: usize = 64;
        const CORE_LEN: usize = 24 + METADATA_LEN + 16; // nonce + meta + tag
        let mut resp = vec![0u8; MAX_PADDING + CORE_LEN + 1024]; // 1024 = max payload
        let mut total = 0usize;
        while total < MAX_PADDING + CORE_LEN {
            let n = stream
                .read(&mut resp[total..])
                .await
                .map_err(|_| Error::Io("mieru out: conn closed"))?;
            if n == 0 {
                return Err(Error::Protocol("mieru out: conn closed"));
            }
            total += n;
        }
        let (seg, _) = parse_segment(&resp[..total], &mut server_cipher, true, true)?;
        let sm = seg
            .session_meta
            .ok_or(Error::Protocol("mieru: expected session meta"))?;
        if sm.protocol_type != OPEN_SESSION_RESPONSE {
            return Err(Error::Protocol("mieru: unexpected response"));
        }

        Ok(Self {
            mieru_session: session,
            client_cipher,
            server_cipher,
            c2s_nonce_sent: true,
            s2c_nonce_recv: true,
        })
    }

    /// Encrypt data for client→server.
    pub fn encrypt_client_data(&mut self, data: &[u8]) -> Result<Vec<u8>, Error> {
        let meta = DataMetadata {
            protocol_type: DATA_CLIENT_TO_SERVER,
            timestamp: MieruSession::timestamp_minutes(),
            session_id: self.mieru_session.session_id,
            sequence_number: self.mieru_session.next_send_seq(),
            unack_sequence: self.mieru_session.peer_unack,
            window_size: self.mieru_session.peer_window,
            fragment_number: 0,
            prefix_length: 0,
            payload_length: data.len() as u16,
            suffix_length: 0,
        };
        let include_nonce = !self.c2s_nonce_sent;
        let seg = build_data_segment(&meta, data, &mut self.client_cipher, include_nonce)?;
        self.c2s_nonce_sent = true;
        Ok(seg)
    }

    /// Decrypt data from server→client.
    pub fn decrypt_server_data(&mut self, data: &[u8]) -> Result<Segment, Error> {
        self.decrypt_server_data_with_consumed(data)
            .map(|(segment, _)| segment)
    }

    pub fn decrypt_server_data_with_consumed(
        &mut self,
        data: &[u8],
    ) -> Result<(Segment, usize), Error> {
        let incl = !self.s2c_nonce_recv;
        let mut server_cipher = self.server_cipher.clone();
        let (seg, consumed) = parse_segment(data, &mut server_cipher, incl, false)?;
        self.server_cipher = server_cipher;
        self.s2c_nonce_recv = true;
        let consumed = consumed.max(segment_wire_len(&seg, incl));
        Ok((seg, consumed))
    }

    /// Build closeSessionRequest.
    pub fn close_request(&mut self) -> Result<Vec<u8>, Error> {
        let meta = SessionMetadata {
            protocol_type: CLOSE_SESSION_REQUEST,
            timestamp: MieruSession::timestamp_minutes(),
            session_id: self.mieru_session.session_id,
            sequence_number: self.mieru_session.next_send_seq(),
            status_code: 0,
            payload_length: 0,
            suffix_length: 0,
        };
        build_session_segment(&meta, &[], &mut self.client_cipher, false)
    }
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

// ── Helpers ──────────────────────────────────────────────────────────

fn encode_target(addr: &Address, port: u16) -> Vec<u8> {
    let mut buf = Vec::new();
    match addr {
        Address::Ipv4(ip) => {
            buf.push(0x01);
            buf.extend_from_slice(ip);
            buf.extend_from_slice(&port.to_be_bytes());
        }
        Address::Ipv6(ip) => {
            buf.push(0x04);
            buf.extend_from_slice(ip);
            buf.extend_from_slice(&port.to_be_bytes());
        }
        Address::Domain(domain) => {
            buf.push(0x03);
            let b = domain.as_bytes();
            buf.push(b.len() as u8);
            buf.extend_from_slice(b);
            buf.extend_from_slice(&port.to_be_bytes());
        }
    }
    buf
}

async fn read_exact<S: AsyncSocket>(
    stream: &mut S,
    buf: &mut [u8],
    len: usize,
) -> Result<(), Error> {
    let mut off = 0;
    while off < len {
        let n = stream
            .read(&mut buf[off..len])
            .await
            .map_err(|_| Error::Io("mieru out read"))?;
        if n == 0 {
            return Err(Error::Protocol("mieru out: conn closed"));
        }
        off += n;
    }
    Ok(())
}

/// Target parameters for Mieru TCP session.
#[derive(Debug, Clone, Copy)]
pub struct MieruTcpTarget<'a> {
    pub target: &'a Address,
    pub port: u16,
    pub username: &'a str,
    pub password: &'a str,
}
