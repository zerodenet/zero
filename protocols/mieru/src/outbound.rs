// Mieru protocol outbound handler — outbound.rs

use alloc::vec::Vec;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{broadcast, mpsc};
use zero_core::{Address, Error};
use zero_traits::AsyncSocket;

use crate::crypto::{derive_key, MieruCipher, NonceConfig};
use crate::metadata::{
    DataMetadata, SessionMetadata, CLOSE_SESSION_REQUEST, DATA_CLIENT_TO_SERVER, METADATA_LEN,
    OPEN_SESSION_REQUEST, OPEN_SESSION_RESPONSE,
};
use crate::segment::{build_data_segment, build_session_segment, parse_segment, Segment};
use crate::session::MieruSession;
use crate::udp;

/// Mieru outbound connection.
pub struct MieruOutbound {
    pub mieru_session: MieruSession,
    pub client_cipher: MieruCipher,
    pub server_cipher: MieruCipher,
    pub c2s_nonce_sent: bool,
    pub s2c_nonce_recv: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MieruUdpFlowPacket {
    pub target: Address,
    pub port: u16,
    pub payload: Vec<u8>,
}

impl MieruUdpFlowPacket {
    pub fn new(target: Address, port: u16, payload: Vec<u8>) -> Self {
        Self {
            target,
            port,
            payload,
        }
    }

    pub fn encode_with(&self, flow_io: &mut MieruUdpFlowIo) -> Result<Vec<u8>, Error> {
        flow_io.encrypt_packet(&self.target, self.port, &self.payload)
    }

    pub fn into_parts(self) -> (Address, u16, Vec<u8>) {
        (self.target, self.port, self.payload)
    }
}

pub struct MieruUdpFlowIo {
    outbound: MieruOutbound,
    recv_raw: Vec<u8>,
}

pub type MieruUdpFlowResponse = (Address, u16, Vec<u8>);

type MieruUdpFlowResponses = broadcast::Sender<MieruUdpFlowResponse>;

pub type MieruUdpFlowResponseReceiver = broadcast::Receiver<MieruUdpFlowResponse>;

#[derive(Clone)]
struct MieruUdpFlowSender {
    send_tx: mpsc::Sender<zero_core::UdpFlowPacket>,
}

pub struct MieruUdpFlowHandle {
    sender: MieruUdpFlowSender,
    responses: MieruUdpFlowResponses,
}

#[derive(Clone)]
pub struct MieruUdpFlowSession {
    sender: MieruUdpFlowSender,
    responses: MieruUdpFlowResponses,
}

impl MieruUdpFlowSession {
    pub fn new(handle: MieruUdpFlowHandle) -> Self {
        Self {
            sender: handle.sender,
            responses: handle.responses,
        }
    }

    pub async fn send(&self, target: &Address, port: u16, payload: &[u8]) -> Result<usize, Error> {
        self.sender.send(target, port, payload).await
    }

    pub fn subscribe_responses(&self) -> MieruUdpFlowResponseReceiver {
        self.responses.subscribe()
    }
}

#[derive(Clone)]
pub struct MieruUdpFlowConnection {
    session: MieruUdpFlowSession,
}

impl MieruUdpFlowConnection {
    pub fn new(session: MieruUdpFlowSession) -> Self {
        Self { session }
    }

    pub async fn send(&self, target: &Address, port: u16, payload: &[u8]) -> Result<usize, Error> {
        self.session.send(target, port, payload).await
    }

    pub fn subscribe_responses(&self) -> MieruUdpFlowResponseReceiver {
        self.session.subscribe_responses()
    }
}

impl MieruUdpFlowSender {
    pub async fn send(&self, target: &Address, port: u16, payload: &[u8]) -> Result<usize, Error> {
        let packet = zero_core::UdpFlowPacket::from_parts(target, port, payload);
        let packet_len = packet.payload.len();
        self.send_tx
            .send(packet)
            .await
            .map_err(|_| Error::Io("mieru udp flow closed"))?;
        Ok(packet_len)
    }
}

impl MieruUdpFlowIo {
    pub async fn establish<S: AsyncSocket>(
        stream: &mut S,
        username: &str,
        password: &str,
    ) -> Result<Self, Error> {
        let mut outbound = MieruOutbound::connect(stream, username, password).await?;
        send_udp_associate_request(stream, &mut outbound).await?;
        read_udp_associate_response(stream, &mut outbound).await?;
        Ok(Self {
            outbound,
            recv_raw: Vec::new(),
        })
    }

    pub async fn establish_with_resume<S>(
        stream: &mut S,
        resume: &crate::udp::MieruUdpFlowResume,
    ) -> Result<Self, Error>
    where
        S: AsyncSocket,
    {
        Self::establish(stream, resume.username(), resume.password()).await
    }

    pub fn encrypt_packet(
        &mut self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, Error> {
        let packet = udp::encode_udp_flow_packet(target, port, payload)?;
        self.encrypt_payload(&packet)
    }

    pub fn encrypt_payload(&mut self, payload: &[u8]) -> Result<Vec<u8>, Error> {
        self.outbound.encrypt_client_data(payload)
    }

    pub fn push_encrypted_response(&mut self, data: &[u8]) {
        self.recv_raw.extend_from_slice(data);
    }

    pub fn next_packet(&mut self) -> Result<Option<MieruUdpFlowPacket>, Error> {
        match self
            .outbound
            .decrypt_server_data_with_consumed(&self.recv_raw)
        {
            Ok((segment, consumed)) => {
                self.recv_raw.drain(..consumed);
                if segment.payload.is_empty() {
                    return Ok(None);
                }
                let packet = udp::decode_udp_flow_packet(&segment.payload)?;
                Ok(Some(MieruUdpFlowPacket::new(
                    packet.target,
                    packet.port,
                    packet.payload,
                )))
            }
            Err(Error::Protocol("mieru: need more data")) => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub fn decode_encrypted_response(
        &mut self,
        data: &[u8],
    ) -> Result<Vec<MieruUdpFlowPacket>, Error> {
        self.push_encrypted_response(data);

        let mut packets = Vec::new();
        while let Some(packet) = self.next_packet()? {
            packets.push(packet);
        }

        Ok(packets)
    }

    pub async fn write_packet<S>(
        &mut self,
        stream: &mut S,
        packet: &MieruUdpFlowPacket,
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        let encrypted = packet.encode_with(self)?;
        stream
            .write_all(&encrypted)
            .await
            .map_err(|_| Error::Io("mieru udp flow write"))
    }

    pub async fn write_flow_packet<S>(
        &mut self,
        stream: &mut S,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        let encrypted = self.encrypt_packet(target, port, payload)?;
        stream
            .write_all(&encrypted)
            .await
            .map_err(|_| Error::Io("mieru udp flow write"))
    }

    pub async fn read_packets<S>(
        &mut self,
        stream: &mut S,
        scratch: &mut [u8],
    ) -> Result<Option<Vec<MieruUdpFlowPacket>>, Error>
    where
        S: AsyncSocket,
    {
        let n = stream
            .read(scratch)
            .await
            .map_err(|_| Error::Io("mieru udp flow read"))?;
        if n == 0 {
            return Ok(None);
        }

        self.push_encrypted_response(&scratch[..n]);

        let mut packets = Vec::new();
        while let Some(packet) = self.next_packet()? {
            packets.push(packet);
        }

        Ok(Some(packets))
    }

    pub async fn read_flow_packets<S>(
        &mut self,
        stream: &mut S,
        scratch: &mut [u8],
    ) -> Result<Option<Vec<(Address, u16, Vec<u8>)>>, Error>
    where
        S: AsyncSocket,
    {
        let Some(packets) = self.read_packets(stream, scratch).await? else {
            return Ok(None);
        };
        Ok(Some(
            packets
                .into_iter()
                .map(MieruUdpFlowPacket::into_parts)
                .collect(),
        ))
    }
}

pub fn spawn_udp_flow<S>(stream: S, flow_io: MieruUdpFlowIo) -> MieruUdpFlowHandle
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    let (send_tx, send_rx) = mpsc::channel::<zero_core::UdpFlowPacket>(32);
    let (responses, _) = broadcast::channel::<MieruUdpFlowResponse>(32);
    spawn_udp_flow_task(stream, flow_io, send_rx, responses.clone());
    MieruUdpFlowHandle {
        sender: MieruUdpFlowSender { send_tx },
        responses,
    }
}

pub async fn establish_udp_flow_with_resume<S>(
    mut stream: S,
    resume: &crate::udp::MieruUdpFlowResume,
) -> Result<MieruUdpFlowConnection, Error>
where
    S: AsyncSocket + AsyncRead + AsyncWrite + Send + 'static,
{
    let flow_io = MieruUdpFlowIo::establish_with_resume(&mut stream, resume).await?;
    Ok(MieruUdpFlowConnection::new(MieruUdpFlowSession::new(
        spawn_udp_flow(stream, flow_io),
    )))
}

fn spawn_udp_flow_task<S>(
    mut stream: S,
    mut flow_io: MieruUdpFlowIo,
    mut send_rx: mpsc::Receiver<zero_core::UdpFlowPacket>,
    responses: MieruUdpFlowResponses,
) where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    tokio::spawn(async move {
        let mut scratch = [0u8; 4096];
        loop {
            tokio::select! {
                to_send = send_rx.recv() => {
                    match to_send {
                        Some(packet) => {
                            let Ok(encrypted) = flow_io.encrypt_packet(
                                &packet.target,
                                packet.port,
                                &packet.payload,
                            ) else {
                                break;
                            };
                            if stream.write_all(&encrypted).await.is_err() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
                read = stream.read(&mut scratch) => {
                    match read {
                        Ok(0) => break,
                        Ok(n) => {
                            let Ok(packets) = flow_io.decode_encrypted_response(&scratch[..n]) else {
                                break;
                            };
                            for packet in packets {
                                let _ = responses.send(packet.into_parts());
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        }
    });
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

async fn send_udp_associate_request<S: AsyncSocket>(
    stream: &mut S,
    outbound: &mut MieruOutbound,
) -> Result<(), Error> {
    let assoc_req = [0x05u8, 0x03, 0x00, 0x01, 0, 0, 0, 0, 0, 0];
    let assoc_seg = outbound.encrypt_client_data(&assoc_req)?;
    stream
        .write_all(&assoc_seg)
        .await
        .map_err(|_| Error::Io("mieru udp assoc write"))?;
    Ok(())
}

async fn read_udp_associate_response<S: AsyncSocket>(
    stream: &mut S,
    outbound: &mut MieruOutbound,
) -> Result<(), Error> {
    let mut assoc_raw = Vec::new();
    let assoc_resp = loop {
        match outbound.decrypt_server_data_with_consumed(&assoc_raw) {
            Ok((segment, consumed)) => {
                assoc_raw.drain(..consumed);
                break segment.payload;
            }
            Err(Error::Protocol("mieru: need more data")) => {
                let mut scratch = [0u8; 4096];
                let n = stream
                    .read(&mut scratch)
                    .await
                    .map_err(|_| Error::Io("mieru udp assoc read"))?;
                if n == 0 {
                    return Err(Error::Protocol("mieru udp assoc: connection closed"));
                }
                assoc_raw.extend_from_slice(&scratch[..n]);
            }
            Err(error) => return Err(error),
        }
    };

    if assoc_resp.len() < 4 || assoc_resp[0] != 0x05 || assoc_resp[1] != 0x00 {
        return Err(Error::Protocol("mieru udp assoc rejected"));
    }

    Ok(())
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
