// Mieru protocol inbound handler — inbound.rs

use alloc::string::String;
use alloc::vec::Vec;

use zero_core::{Address, Error, Network, ProtocolType, Session};
use zero_traits::AsyncSocket;

use crate::crypto::{try_derive_keys, MieruCipher};
use crate::metadata::{SessionMetadata, METADATA_LEN, OPEN_SESSION_REQUEST, OPEN_SESSION_RESPONSE};
use crate::segment::build_session_segment;
use crate::session::MieruSession;

/// Mieru inbound handler.
#[derive(Debug, Default, Clone)]
pub struct MieruInbound;

/// Result of accepting a mieru TCP connection.
pub struct MieruAccept {
    pub session: Session,
    pub mieru_session: MieruSession,
    pub client_cipher: MieruCipher,
    pub server_cipher: MieruCipher,
    pub remaining_payload: Vec<u8>,
}

impl MieruInbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Trojan
    }

    /// Accept a mieru TCP connection — perform full handshake.
    pub async fn accept_request<S: AsyncSocket>(
        &self,
        stream: &mut S,
        users: &[(String, String)],
    ) -> Result<MieruAccept, Error> {
        // Read first segment: nonce(24) + encrypted_meta(32) + tag(16) = 72 bytes
        let mut first = vec![0u8; 72];
        read_exact(stream, &mut first, 72).await?;

        let unix_now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| Error::Protocol("mieru: system time error"))?
            .as_secs();

        // Try each user's key
        let mut matched: Option<(&str, MieruCipher, MieruCipher, SessionMetadata)> = None;

        for (username, password) in users {
            let keys = try_derive_keys(username, password, unix_now);
            for key in &keys {
                let mut c = MieruCipher::new(key);
                if let Ok(pt) = c.decrypt(true, &first) {
                    if pt.len() >= METADATA_LEN {
                        let meta = SessionMetadata::decode(&pt[..METADATA_LEN]);
                        if meta.protocol_type == OPEN_SESSION_REQUEST {
                            matched = Some((username.as_str(), c, MieruCipher::new(key), meta));
                            break;
                        }
                    }
                }
            }
            if matched.is_some() {
                break;
            }
        }

        let (_username, mut client_cipher, mut server_cipher, open_req) =
            matched.ok_or(Error::Protocol("mieru: no valid user key found"))?;

        // Decrypt payload from openSessionRequest
        let (target, port, remaining) = if open_req.payload_length > 0 {
            let plen = open_req.payload_length as usize;
            let mut payload_ct = vec![0u8; plen + 16]; // ciphertext + tag
            read_exact(stream, &mut payload_ct, plen + 16).await?;
            let payload_pt = client_cipher.decrypt(false, &payload_ct)?;
            parse_target(&payload_pt)?
        } else {
            return Err(Error::Protocol("mieru: openSessionRequest missing target"));
        };

        // Send openSessionResponse
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
            session: Session::new(0, target, port, Network::Tcp, ProtocolType::Trojan),
            mieru_session: session,
            client_cipher,
            server_cipher,
            remaining_payload: remaining,
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

fn parse_target(data: &[u8]) -> Result<(Address, u16, Vec<u8>), Error> {
    if data.is_empty() {
        return Err(Error::Protocol("mieru: empty target"));
    }
    match data[0] {
        0x01 => {
            if data.len() < 7 {
                return Err(Error::Protocol("mieru: ipv4 truncated"));
            }
            let mut ip = [0u8; 4];
            ip.copy_from_slice(&data[1..5]);
            let port = u16::from_be_bytes([data[5], data[6]]);
            Ok((Address::Ipv4(ip), port, data[7..].to_vec()))
        }
        0x03 => {
            if data.len() < 5 {
                return Err(Error::Protocol("mieru: domain truncated"));
            }
            let dlen = data[1] as usize;
            let end = 2 + dlen;
            if data.len() < end + 2 {
                return Err(Error::Protocol("mieru: domain truncated"));
            }
            let domain = String::from_utf8_lossy(&data[2..end]).to_string();
            let port = u16::from_be_bytes([data[end], data[end + 1]]);
            Ok((Address::Domain(domain), port, data[end + 2..].to_vec()))
        }
        0x04 => {
            if data.len() < 19 {
                return Err(Error::Protocol("mieru: ipv6 truncated"));
            }
            let mut ip = [0u8; 16];
            ip.copy_from_slice(&data[1..17]);
            let port = u16::from_be_bytes([data[17], data[18]]);
            Ok((Address::Ipv6(ip), port, data[19..].to_vec()))
        }
        _ => Err(Error::Protocol("mieru: unknown addr type")),
    }
}
