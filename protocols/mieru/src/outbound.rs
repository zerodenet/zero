// Mieru protocol outbound handler — outbound.rs

use alloc::vec::Vec;

use zero_core::Error;
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
    ///
    /// Establishes the encrypted mieru session only. The session is a raw
    /// encrypted tunnel and does NOT carry a target — upstream mieru conveys
    /// the proxy target via socks5 running inside the tunnel (mita runs a
    /// socks5 server on the decrypted session). Callers must perform that
    /// socks5 handshake over the resulting stream to bind a target.
    pub async fn connect<S: AsyncSocket>(
        stream: &mut S,
        username: &str,
        password: &str,
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

        // openSessionRequest carries only the session ID — no target payload.
        let open_meta = SessionMetadata {
            protocol_type: OPEN_SESSION_REQUEST,
            timestamp: MieruSession::timestamp_minutes(),
            session_id: session.session_id,
            sequence_number: 0,
            status_code: 0,
            payload_length: 0,
            suffix_length: 0,
        };
        let open_seg = build_session_segment(&open_meta, &[], &mut client_cipher, true)?;
        stream
            .write_all(&open_seg)
            .await
            .map_err(|_| Error::Io("mieru: send open"))?;

        // Read openSessionResponse. Upstream emits no leading padding0, so the
        // nonce + encrypted metadata is exactly CORE_LEN bytes at offset 0.
        const CORE_LEN: usize = 24 + METADATA_LEN + 16; // nonce + meta + tag
        let mut resp = vec![0u8; CORE_LEN];
        read_exact(stream, &mut resp, CORE_LEN).await?;
        let (seg, _) = parse_segment(&resp, &mut server_cipher, true, true)?;
        let sm = seg
            .session_meta
            .ok_or(Error::Protocol("mieru: expected session meta"))?;
        if sm.protocol_type != OPEN_SESSION_RESPONSE {
            return Err(Error::Protocol("mieru: unexpected response"));
        }

        // Consume any suffix padding declared by the response so the stream is
        // cleanly positioned for the data (socks5) phase.
        if sm.suffix_length > 0 {
            let mut suffix = vec![0u8; sm.suffix_length as usize];
            read_exact(stream, &mut suffix, sm.suffix_length as usize).await?;
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

/// Credential parameters for a Mieru outbound session.
///
/// The mieru session is target-agnostic (it is an encrypted tunnel); the
/// proxy target is conveyed by a socks5 handshake the caller runs over the
/// established session, matching upstream mieru.
#[derive(Debug, Clone, Copy)]
pub struct MieruTcpTarget<'a> {
    pub username: &'a str,
    pub password: &'a str,
}
