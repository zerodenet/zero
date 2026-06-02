// Mieru protocol outbound handler — outbound.rs

use alloc::vec::Vec;

use zero_core::{Address, Error};
use zero_traits::AsyncSocket;

use crate::crypto::{derive_key, MieruCipher};
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
        let mut client_cipher = MieruCipher::new(&key);
        let mut server_cipher = MieruCipher::new(&key);
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

        // Read openSessionResponse: nonce(24) + meta(32) + tag(16) = 72
        let mut resp = vec![0u8; 72];
        read_exact(stream, &mut resp, 72).await?;
        let (seg, _) = parse_segment(&resp, &mut server_cipher, true, true)?;
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
        let incl = !self.s2c_nonce_recv;
        let (seg, _) = parse_segment(data, &mut self.server_cipher, incl, false)?;
        self.s2c_nonce_recv = true;
        Ok(seg)
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
