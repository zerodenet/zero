use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::sync::mpsc;
use zero_core::{Address, Error, Network, Session};
use zero_traits::AsyncSocket;

use crate::outbound::VmessOutbound;
use crate::shared::{parse_address_from_bytes, read_exact, write_address};
use crate::stream::VmessAeadStream;

pub const MUX_MAX_META_LEN: usize = 512;
pub const MUX_MAX_DATA_LEN: usize = 16 * 1024;
pub const MUX_NETWORK_TCP: u8 = 0x01;
pub const MUX_NETWORK_UDP: u8 = 0x02;
pub const MUX_STATUS_NEW: u8 = 0x01;
pub const MUX_STATUS_KEEP: u8 = 0x02;
pub const MUX_STATUS_END: u8 = 0x03;
pub const MUX_STATUS_KEEP_ALIVE: u8 = 0x04;
pub const MUX_OPTION_DATA: u8 = 0x01;
pub const MUX_OPTION_ERROR: u8 = 0x02;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MuxFrame {
    pub session_id: u16,
    pub status: u8,
    pub option: u8,
    pub network: Option<Network>,
    pub target: Option<Address>,
    pub port: Option<u16>,
    pub payload: Vec<u8>,
}

pub fn mux_cool_session() -> Session {
    Session::new(
        0,
        Address::Domain(crate::shared::MUX_COOL_DOMAIN.to_owned()),
        crate::shared::MUX_COOL_PORT,
        Network::Tcp,
        zero_core::ProtocolType::Vmess,
    )
}

pub fn is_mux_cool_session(session: &Session) -> bool {
    matches!(&session.target, Address::Domain(domain) if domain == crate::shared::MUX_COOL_DOMAIN)
        && session.port == crate::shared::MUX_COOL_PORT
        && session.network == Network::Tcp
}

pub fn encode_frame(
    session_id: u16,
    status: u8,
    option: u8,
    target: Option<(&Address, u16, Network)>,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    let mut meta = Vec::new();
    meta.extend_from_slice(&session_id.to_be_bytes());
    meta.push(status);
    meta.push(option);

    if status == MUX_STATUS_NEW {
        let Some((address, port, network)) = target else {
            return Err(Error::Protocol("vmess mux new frame requires target"));
        };
        match network {
            Network::Tcp => meta.push(MUX_NETWORK_TCP),
            Network::Udp => meta.push(MUX_NETWORK_UDP),
        }
        meta.extend_from_slice(&port.to_be_bytes());
        write_address(&mut meta, address)?;
    }

    if meta.len() > MUX_MAX_META_LEN {
        return Err(Error::Protocol("vmess mux metadata too large"));
    }

    let mut frame = Vec::with_capacity(2 + meta.len() + 2 + payload.len());
    frame.extend_from_slice(&(meta.len() as u16).to_be_bytes());
    frame.extend_from_slice(&meta);
    if option & MUX_OPTION_DATA != 0 {
        if payload.len() > MUX_MAX_DATA_LEN {
            return Err(Error::Protocol("vmess mux payload too large"));
        }
        frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        frame.extend_from_slice(payload);
    }
    Ok(frame)
}

pub async fn read_frame<S: AsyncSocket>(stream: &mut S) -> Result<MuxFrame, Error> {
    let mut len_buf = [0u8; 2];
    read_exact(stream, &mut len_buf).await?;
    let meta_len = u16::from_be_bytes(len_buf) as usize;
    if meta_len > MUX_MAX_META_LEN {
        return Err(Error::Protocol("vmess mux metadata too large"));
    }

    let mut meta = vec![0_u8; meta_len];
    read_exact(stream, &mut meta).await?;
    let mut frame = decode_metadata(&meta)?;

    if frame.option & MUX_OPTION_DATA != 0 {
        read_exact(stream, &mut len_buf).await?;
        let data_len = u16::from_be_bytes(len_buf) as usize;
        if data_len > MUX_MAX_DATA_LEN {
            return Err(Error::Protocol("vmess mux data too large"));
        }
        frame.payload.resize(data_len, 0);
        if data_len > 0 {
            read_exact(stream, &mut frame.payload).await?;
        }
    }

    Ok(frame)
}

pub async fn read_frame_from_tokio<R>(reader: &mut R) -> Result<MuxFrame, Error>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut len_buf = [0u8; 2];
    tokio::io::AsyncReadExt::read_exact(reader, &mut len_buf)
        .await
        .map_err(|_| Error::Io("vmess: failed to read from socket"))?;
    let meta_len = u16::from_be_bytes(len_buf) as usize;
    if meta_len > MUX_MAX_META_LEN {
        return Err(Error::Protocol("vmess mux metadata too large"));
    }

    let mut meta = vec![0_u8; meta_len];
    tokio::io::AsyncReadExt::read_exact(reader, &mut meta)
        .await
        .map_err(|_| Error::Io("vmess: failed to read from socket"))?;
    let mut frame = decode_metadata(&meta)?;

    if frame.option & MUX_OPTION_DATA != 0 {
        tokio::io::AsyncReadExt::read_exact(reader, &mut len_buf)
            .await
            .map_err(|_| Error::Io("vmess: failed to read from socket"))?;
        let data_len = u16::from_be_bytes(len_buf) as usize;
        if data_len > MUX_MAX_DATA_LEN {
            return Err(Error::Protocol("vmess mux data too large"));
        }
        frame.payload.resize(data_len, 0);
        if data_len > 0 {
            tokio::io::AsyncReadExt::read_exact(reader, &mut frame.payload)
                .await
                .map_err(|_| Error::Io("vmess: failed to read from socket"))?;
        }
    }

    Ok(frame)
}

pub async fn read_mux_stream_frame<R>(reader: &mut R) -> Result<MuxFrame, Error>
where
    R: tokio::io::AsyncRead + Unpin,
{
    read_frame_from_tokio(reader).await
}

pub fn decode_metadata(meta: &[u8]) -> Result<MuxFrame, Error> {
    if meta.len() < 4 {
        return Err(Error::Protocol("vmess mux metadata too short"));
    }

    let session_id = u16::from_be_bytes([meta[0], meta[1]]);
    let status = meta[2];
    let option = meta[3];

    let mut frame = MuxFrame {
        session_id,
        status,
        option,
        network: None,
        target: None,
        port: None,
        payload: Vec::new(),
    };

    if status == MUX_STATUS_NEW {
        if meta.len() < 8 {
            return Err(Error::Protocol("vmess mux new metadata too short"));
        }
        frame.network = match meta[4] {
            MUX_NETWORK_TCP => Some(Network::Tcp),
            MUX_NETWORK_UDP => Some(Network::Udp),
            _ => return Err(Error::Protocol("vmess mux unknown network")),
        };
        frame.port = Some(u16::from_be_bytes([meta[5], meta[6]]));
        frame.target = Some(parse_address_from_bytes(meta[7], &meta[8..])?);
    }

    Ok(frame)
}

pub fn encode_open_stream(
    session_id: u16,
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    encode_open_stream_with_network(session_id, target, port, Network::Tcp, payload)
}

pub fn encode_open_stream_with_network(
    session_id: u16,
    target: &Address,
    port: u16,
    network: Network,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    let option = if payload.is_empty() {
        0
    } else {
        MUX_OPTION_DATA
    };
    encode_frame(
        session_id,
        MUX_STATUS_NEW,
        option,
        Some((target, port, network)),
        payload,
    )
}

pub fn encode_keep_stream(session_id: u16, payload: &[u8]) -> Result<Vec<u8>, Error> {
    encode_frame(session_id, MUX_STATUS_KEEP, MUX_OPTION_DATA, None, payload)
}

pub fn encode_end_stream(session_id: u16) -> Result<Vec<u8>, Error> {
    encode_frame(session_id, MUX_STATUS_END, 0, None, &[])
}

pub struct VmessMuxStream {
    session_id: u16,
    target: Address,
    port: u16,
    network: Network,
    write_tx: mpsc::UnboundedSender<Vec<u8>>,
    read_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    write_buf: Vec<u8>,
    write_pos: usize,
    read_buf: Vec<u8>,
    read_pos: usize,
    opened: bool,
    ended: bool,
    active: Option<Arc<Mutex<usize>>>,
}

pub struct VmessMuxConn {
    write_tx: mpsc::UnboundedSender<Vec<u8>>,
    streams: Arc<Mutex<std::collections::HashMap<u16, mpsc::UnboundedSender<Vec<u8>>>>>,
    next_id: Mutex<u16>,
    active: Arc<Mutex<usize>>,
    max_concurrency: u32,
}

impl VmessMuxConn {
    pub fn new<S>(stream: S, max_concurrency: u32) -> Self
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let (reader, writer) = tokio::io::split(stream);
        let (write_tx, write_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let streams = Arc::new(Mutex::new(std::collections::HashMap::new()));

        spawn_mux_write_relay(writer, write_rx);
        spawn_mux_read_relay(reader, streams.clone());

        Self {
            write_tx,
            streams,
            next_id: Mutex::new(1),
            active: Arc::new(Mutex::new(0)),
            max_concurrency,
        }
    }

    pub fn has_capacity(&self) -> bool {
        *self.active.lock().unwrap() < self.max_concurrency as usize
    }

    pub fn open_stream(&self, target: Address, port: u16, network: Network) -> VmessMuxStream {
        let session_id = self.allocate_stream_id();
        let (down_tx, down_rx) = mpsc::unbounded_channel();
        self.streams.lock().unwrap().insert(session_id, down_tx);

        VmessMuxStream::new_with_network(
            session_id,
            target,
            port,
            network,
            self.write_tx.clone(),
            down_rx,
            self.active.clone(),
        )
    }

    fn allocate_stream_id(&self) -> u16 {
        let session_id = {
            let mut next = self.next_id.lock().unwrap();
            let id = *next;
            *next = next.wrapping_add(1);
            if *next == 0 {
                *next = 1;
            }
            id
        };
        *self.active.lock().unwrap() += 1;
        session_id
    }
}

fn spawn_mux_write_relay<W>(mut writer: W, mut write_rx: mpsc::UnboundedReceiver<Vec<u8>>)
where
    W: AsyncWrite + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        while let Some(frame) = write_rx.recv().await {
            if writer.write_all(&frame).await.is_err() {
                break;
            }
            if writer.flush().await.is_err() {
                break;
            }
        }
        let _ = writer.shutdown().await;
    });
}

fn spawn_mux_read_relay<R>(
    mut reader: R,
    streams: Arc<Mutex<std::collections::HashMap<u16, mpsc::UnboundedSender<Vec<u8>>>>>,
) where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        loop {
            let frame = match read_mux_stream_frame(&mut reader).await {
                Ok(frame) => frame,
                Err(_) => break,
            };
            if frame.status == MUX_STATUS_KEEP_ALIVE {
                continue;
            }
            let tx = streams.lock().unwrap().get(&frame.session_id).cloned();
            if let Some(tx) = tx {
                if frame.status == MUX_STATUS_END {
                    let _ = tx.send(Vec::new());
                    streams.lock().unwrap().remove(&frame.session_id);
                } else if !frame.payload.is_empty() {
                    let _ = tx.send(frame.payload);
                }
            }
        }
    });
}

impl VmessMuxStream {
    pub fn new(
        session_id: u16,
        target: Address,
        port: u16,
        write_tx: mpsc::UnboundedSender<Vec<u8>>,
        read_rx: mpsc::UnboundedReceiver<Vec<u8>>,
        active: Arc<Mutex<usize>>,
    ) -> Self {
        Self::new_with_network(
            session_id,
            target,
            port,
            Network::Tcp,
            write_tx,
            read_rx,
            active,
        )
    }

    pub fn new_with_network(
        session_id: u16,
        target: Address,
        port: u16,
        network: Network,
        write_tx: mpsc::UnboundedSender<Vec<u8>>,
        read_rx: mpsc::UnboundedReceiver<Vec<u8>>,
        active: Arc<Mutex<usize>>,
    ) -> Self {
        Self {
            session_id,
            target,
            port,
            network,
            write_tx,
            read_rx,
            write_buf: Vec::new(),
            write_pos: 0,
            read_buf: Vec::new(),
            read_pos: 0,
            opened: false,
            ended: false,
            active: Some(active),
        }
    }

    fn queue_frame(&mut self, payload: &[u8]) -> io::Result<usize> {
        let take = payload.len().min(MUX_MAX_DATA_LEN);
        let frame = if self.opened {
            encode_keep_stream(self.session_id, &payload[..take])
        } else {
            self.opened = true;
            encode_open_stream_with_network(
                self.session_id,
                &self.target,
                self.port,
                self.network,
                &payload[..take],
            )
        }
        .map_err(protocol_error)?;
        self.write_tx
            .send(frame)
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "vmess mux writer closed"))?;
        Ok(take)
    }

    fn flush_pending(&mut self) -> io::Result<()> {
        if self.write_pos < self.write_buf.len() {
            self.write_tx
                .send(self.write_buf[self.write_pos..].to_vec())
                .map_err(|_| {
                    io::Error::new(io::ErrorKind::BrokenPipe, "vmess mux writer closed")
                })?;
            self.write_pos = self.write_buf.len();
        }
        self.write_buf.clear();
        self.write_pos = 0;
        Ok(())
    }
}

pub fn mux_stream_with_network(
    session_id: u16,
    target: Address,
    port: u16,
    network: Network,
    write_tx: mpsc::UnboundedSender<Vec<u8>>,
    read_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    active: Arc<Mutex<usize>>,
) -> VmessMuxStream {
    VmessMuxStream::new_with_network(session_id, target, port, network, write_tx, read_rx, active)
}

pub async fn establish_mux_outbound_stream<S>(
    mut stream: S,
    uuid: &[u8; 16],
    cipher: crate::shared::VmessCipher,
) -> Result<VmessAeadStream<S>, Error>
where
    S: AsyncSocket,
{
    let mux_session = VmessOutbound
        .establish_tcp_session(&mut stream, &mux_cool_session(), uuid, cipher)
        .await?;
    VmessAeadStream::outbound(stream, mux_session)
}

impl Drop for VmessMuxStream {
    fn drop(&mut self) {
        if !self.ended {
            if !self.opened {
                let _ = self.write_tx.send(
                    encode_open_stream_with_network(
                        self.session_id,
                        &self.target,
                        self.port,
                        self.network,
                        &[],
                    )
                    .unwrap_or_default(),
                );
            }
            let _ = self
                .write_tx
                .send(encode_end_stream(self.session_id).unwrap_or_default());
            self.ended = true;
        }
        if let Some(active) = self.active.take() {
            if let Ok(mut count) = active.lock() {
                *count = count.saturating_sub(1);
            }
        }
    }
}

impl AsyncRead for VmessMuxStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.read_pos < self.read_buf.len() {
            let n = (self.read_buf.len() - self.read_pos).min(buf.remaining());
            buf.put_slice(&self.read_buf[self.read_pos..self.read_pos + n]);
            self.read_pos += n;
            if self.read_pos == self.read_buf.len() {
                self.read_buf.clear();
                self.read_pos = 0;
            }
            return Poll::Ready(Ok(()));
        }

        match Pin::new(&mut self.read_rx).poll_recv(cx) {
            Poll::Ready(Some(chunk)) => {
                if chunk.is_empty() {
                    self.ended = true;
                    return Poll::Ready(Ok(()));
                }
                let n = chunk.len().min(buf.remaining());
                buf.put_slice(&chunk[..n]);
                if n < chunk.len() {
                    self.read_buf = chunk;
                    self.read_pos = n;
                }
                Poll::Ready(Ok(()))
            }
            Poll::Ready(None) => {
                self.ended = true;
                Poll::Ready(Ok(()))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl AsyncWrite for VmessMuxStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if let Err(error) = self.flush_pending() {
            return Poll::Ready(Err(error));
        }
        if buf.is_empty() {
            return Poll::Ready(Ok(0));
        }
        Poll::Ready(self.queue_frame(buf))
    }

    fn poll_flush(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(self.flush_pending())
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if let Err(error) = self.flush_pending() {
            return Poll::Ready(Err(error));
        }
        if !self.ended {
            if !self.opened {
                match encode_open_stream_with_network(
                    self.session_id,
                    &self.target,
                    self.port,
                    self.network,
                    &[],
                ) {
                    Ok(frame) => {
                        if self.write_tx.send(frame).is_err() {
                            return Poll::Ready(Err(io::Error::new(
                                io::ErrorKind::BrokenPipe,
                                "vmess mux writer closed",
                            )));
                        }
                        self.opened = true;
                    }
                    Err(error) => return Poll::Ready(Err(protocol_error(error))),
                }
            }
            match encode_end_stream(self.session_id) {
                Ok(frame) => {
                    if self.write_tx.send(frame).is_err() {
                        return Poll::Ready(Err(io::Error::new(
                            io::ErrorKind::BrokenPipe,
                            "vmess mux writer closed",
                        )));
                    }
                }
                Err(error) => return Poll::Ready(Err(protocol_error(error))),
            }
            self.ended = true;
        }
        Poll::Ready(Ok(()))
    }
}

fn protocol_error(error: Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}
