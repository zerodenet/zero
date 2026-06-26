// Hysteria2 QUIC stream wrapper — stream.rs
//
// Wraps a quinn SendStream + RecvStream into a single bidirectional
// AsyncRead + AsyncWrite for use by the proxy relay layer.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};

use zero_traits::AsyncSocket;

/// Bidirectional QUIC stream for Hysteria2 TCP relay.
pub struct Hysteria2Stream {
    send: quinn::SendStream,
    recv: quinn::RecvStream,
}

impl Hysteria2Stream {
    pub fn new(send: quinn::SendStream, recv: quinn::RecvStream) -> Self {
        Self { send, recv }
    }
}

impl AsyncRead for Hysteria2Stream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.recv).poll_read(cx, buf)
    }
}

impl AsyncWrite for Hysteria2Stream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.send)
            .poll_write(cx, buf)
            .map_err(io::Error::other)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.send)
            .poll_flush(cx)
            .map_err(io::Error::other)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.send)
            .poll_shutdown(cx)
            .map_err(io::Error::other)
    }
}

impl AsyncSocket for Hysteria2Stream {
    type Error = io::Error;

    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        AsyncReadExt::read(self, buf).await
    }

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        AsyncWriteExt::write_all(self, buf).await?;
        AsyncWriteExt::flush(self).await
    }

    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        AsyncWriteExt::shutdown(self).await
    }
}

// ── Hysteria2Connector ──

use hysteria2::{build_auth_frame, build_tcp_connect_header, parse_auth_response, sign_hmac};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use zero_core::{Address, Error, Session, UdpFlowPacket};
use zero_engine::EngineError;

/// Establishes a Hysteria2 outbound connection.
///
/// Handles the full flow: QUIC connect → HMAC auth → TCP stream setup.
/// Implements the same pattern as [`VlessTransportConnector`].
pub struct Hysteria2Connector {
    server: String,
    port: u16,
    password: String,
    client_fingerprint: Option<String>,
}

impl Hysteria2Connector {
    pub fn new(server: &str, port: u16, password: &str) -> Self {
        Self {
            server: server.to_owned(),
            port,
            password: password.to_owned(),
            client_fingerprint: None,
        }
    }

    pub fn with_fingerprint(mut self, fp: Option<&str>) -> Self {
        self.client_fingerprint = fp.map(|s| s.to_owned());
        self
    }

    /// Connect + authenticate, returning the raw QUIC connection.
    pub async fn connect_raw(&self) -> Result<quinn::Connection, EngineError> {
        // Build TLS config with optional fingerprint
        let config_base = if let Some(ref fp_name) = self.client_fingerprint {
            if let Some(preset) = crate::fingerprint::lookup_fingerprint(fp_name) {
                let provider = std::sync::Arc::new(crate::fingerprint::build_provider(&preset));
                tracing::debug!(
                    fingerprint = %fp_name,
                    "hysteria2 tls fingerprint applied"
                );
                rustls::ClientConfig::builder_with_provider(provider)
                    .with_protocol_versions(&[&rustls::version::TLS13, &rustls::version::TLS12])
                    .map_err(|e| {
                        EngineError::Io(std::io::Error::other(format!(
                            "hysteria2 tls protocol: {e}"
                        )))
                    })?
            } else {
                tracing::warn!(
                    fingerprint = %fp_name,
                    "unknown hysteria2 tls fingerprint, using defaults"
                );
                rustls::ClientConfig::builder()
            }
        } else {
            rustls::ClientConfig::builder()
        };

        let mut tls_config = config_base
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(SkipVerify))
            .with_no_client_auth();
        tls_config.alpn_protocols = vec![b"hysteria2".to_vec()];

        let quic_cfg =
            quinn::crypto::rustls::QuicClientConfig::try_from(tls_config).map_err(|e| {
                EngineError::Io(std::io::Error::other(format!("hysteria2 tls cfg: {e}")))
            })?;

        let mut client_cfg = quinn::ClientConfig::new(Arc::new(quic_cfg));
        let mut transport = quinn::TransportConfig::default();
        transport.max_idle_timeout(Some(std::time::Duration::from_secs(30).try_into().unwrap()));
        transport.datagram_receive_buffer_size(Some(65536));
        client_cfg.transport_config(Arc::new(transport));

        let bind_addr: std::net::SocketAddr = "0.0.0.0:0".parse().map_err(|e| {
            EngineError::Io(std::io::Error::other(format!("hysteria2 bind addr: {e}")))
        })?;
        let socket = std::net::UdpSocket::bind(bind_addr).map_err(|e| {
            EngineError::Io(std::io::Error::other(format!("hysteria2 bind socket: {e}")))
        })?;
        let mut endpoint = quinn::Endpoint::new(
            quinn::EndpointConfig::default(),
            None,
            socket,
            Arc::new(quinn::TokioRuntime),
        )
        .map_err(|e| EngineError::Io(std::io::Error::other(format!("hysteria2 endpoint: {e}"))))?;
        endpoint.set_default_client_config(client_cfg);

        let server_addr = format!("{}:{}", self.server, self.port)
            .parse::<std::net::SocketAddr>()
            .map_err(|e| EngineError::Io(std::io::Error::other(format!("hysteria2 addr: {e}"))))?;

        let conn = endpoint
            .connect(server_addr, &self.server)
            .map_err(|e| EngineError::Io(std::io::Error::other(format!("hysteria2 connect: {e}"))))?
            .await
            .map_err(|e| {
                EngineError::Io(std::io::Error::other(format!("hysteria2 connection: {e}")))
            })?;

        // HMAC auth is bound to this QUIC connection.
        let mut salt = [0u8; 32];
        conn.export_keying_material(&mut salt, b"hysteria2 auth", &[])
            .map_err(|_| EngineError::Io(std::io::Error::other("hysteria2 key export failed")))?;
        let hmac_bytes = sign_hmac(&self.password, &salt);

        let (mut send, mut recv) = conn.open_bi().await.map_err(|e| {
            EngineError::Io(std::io::Error::other(format!("hysteria2 open_bi: {e}")))
        })?;

        let auth_frame = build_auth_frame(&hmac_bytes);
        send.write_all(&auth_frame)
            .await
            .map_err(|e| EngineError::Io(e.into()))?;

        let mut resp_buf = [0u8; 32];
        let n = recv
            .read(&mut resp_buf)
            .await
            .map_err(|e| {
                EngineError::Io(std::io::Error::other(format!("hysteria2 auth read: {e}")))
            })?
            .unwrap_or(0);
        parse_auth_response(&resp_buf[..n]).map_err(|e| {
            EngineError::Io(std::io::Error::other(format!("hysteria2 auth failed: {e}")))
        })?;

        drop(send);
        drop(recv);

        Ok(conn)
    }

    /// Establish a Hysteria2 TCP connection (QUIC connect + auth + TCP stream).
    pub async fn connect(&self, session: &Session) -> Result<Hysteria2Stream, EngineError> {
        let conn = self.connect_raw().await?;

        let (mut send, mut recv) = conn.open_bi().await.map_err(|e| {
            EngineError::Io(std::io::Error::other(format!("hysteria2 open_bi: {e}")))
        })?;

        // TCP connect
        let connect_header =
            build_tcp_connect_header(&session.target, session.port).map_err(|e| {
                EngineError::Io(std::io::Error::other(format!(
                    "hysteria2 connect header: {e}"
                )))
            })?;
        send.write_all(&connect_header)
            .await
            .map_err(|e| EngineError::Io(e.into()))?;
        send.flush().await.map_err(EngineError::Io)?;

        let mut ok_buf = [0u8; 1];
        recv.read_exact(&mut ok_buf).await.map_err(|e| {
            EngineError::Io(std::io::Error::other(format!(
                "hysteria2 connect read: {e}"
            )))
        })?;
        if ok_buf[0] != 0x01 {
            return Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                "hysteria2: connect rejected",
            )));
        }

        Ok(Hysteria2Stream::new(send, recv))
    }
}

pub type Hysteria2UdpResponse = (Address, u16, Vec<u8>);

pub struct Hysteria2UdpFlowStream {
    pub send_tx: mpsc::Sender<UdpFlowPacket>,
    pub recv_tx: broadcast::Sender<Hysteria2UdpResponse>,
}

pub struct Hysteria2UdpFlowStreamRequest {
    pub server: String,
    pub port: u16,
    pub resume: hysteria2::Hysteria2UdpFlowResume,
    pub initial_packet: UdpFlowPacket,
}

pub async fn establish_hysteria2_udp_flow_stream(
    request: Hysteria2UdpFlowStreamRequest,
) -> Result<Hysteria2UdpFlowStream, EngineError> {
    let connector_profile = request.resume.connector_profile();
    let connector =
        Hysteria2Connector::new(&request.server, request.port, connector_profile.password())
            .with_fingerprint(connector_profile.client_fingerprint());
    let conn = Arc::new(connector.connect_raw().await?);

    let (send_tx, send_rx) = mpsc::channel::<UdpFlowPacket>(32);
    let (recv_tx, _) = broadcast::channel::<Hysteria2UdpResponse>(32);

    spawn_hysteria2_udp_send_task(
        conn.clone(),
        send_rx,
        request.initial_packet,
        request.resume.clone(),
    );
    spawn_hysteria2_udp_recv_task(conn, recv_tx.clone(), request.resume);

    Ok(Hysteria2UdpFlowStream { send_tx, recv_tx })
}

fn spawn_hysteria2_udp_send_task(
    conn: Arc<quinn::Connection>,
    mut send_rx: mpsc::Receiver<UdpFlowPacket>,
    initial_packet: UdpFlowPacket,
    resume: hysteria2::Hysteria2UdpFlowResume,
) {
    tokio::spawn(async move {
        if let Ok(datagram) = encode_hysteria2_udp_flow_packet(initial_packet, &resume) {
            if conn.send_datagram(datagram.into()).is_err() {
                return;
            }
        }
        while let Some(packet) = send_rx.recv().await {
            let Ok(datagram) = encode_hysteria2_udp_flow_packet(packet, &resume) else {
                break;
            };
            if conn.send_datagram(datagram.into()).is_err() {
                break;
            }
        }
    });
}

fn encode_hysteria2_udp_flow_packet(
    packet: UdpFlowPacket,
    resume: &hysteria2::Hysteria2UdpFlowResume,
) -> Result<Vec<u8>, Error> {
    let packet = hysteria2::udp_flow_packet(&packet.target, packet.port, &packet.payload);
    packet.encode_with(resume)
}

fn spawn_hysteria2_udp_recv_task(
    conn: Arc<quinn::Connection>,
    recv_tx: broadcast::Sender<Hysteria2UdpResponse>,
    resume: hysteria2::Hysteria2UdpFlowResume,
) {
    tokio::spawn(async move {
        while let Ok(data) = conn.read_datagram().await {
            if let Some(packet) = resume.decode_flow_packet(&data) {
                let (target, port, payload) = packet.into_parts();
                if recv_tx.send((target, port, payload)).is_err() {
                    break;
                }
            }
        }
    });
}

#[derive(Debug)]
struct SkipVerify;

impl rustls::client::danger::ServerCertVerifier for SkipVerify {
    fn verify_server_cert(
        &self,
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &[rustls::pki_types::CertificateDer<'_>],
        _: &rustls::pki_types::ServerName<'_>,
        _: &[u8],
        _: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        _: &[u8],
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(
        &self,
        _: &[u8],
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ED25519,
        ]
    }
}
