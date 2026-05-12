// QUIC transport — quic.rs
//
// UDP-based transport with TLS 1.3 encryption built-in via QUIC.
// Uses quinn (Rust QUIC implementation).

use std::io;
#[cfg(feature = "inbound-socks5")]
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};

use zero_engine::EngineError;
use zero_traits::AsyncSocket;

use super::ClientStream;

/// Bidirectional QUIC stream wrapping quinn SendStream and RecvStream.
pub(crate) struct QuicStream {
    send: quinn::SendStream,
    recv: quinn::RecvStream,
}

impl QuicStream {
    fn new(send: quinn::SendStream, recv: quinn::RecvStream) -> Self {
        Self { send, recv }
    }
}

// ── client (outbound) connect ──

#[cfg(feature = "outbound-vless")]
pub(crate) async fn connect_quic(
    server_name: &str,
    port: u16,
    _insecure: bool,
) -> Result<QuicStream, EngineError> {
    use quinn::crypto::rustls::QuicClientConfig;

    let mut tls_config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(SkipServerVerification::new())
        .with_no_client_auth();

    tls_config.alpn_protocols = vec![b"h3".to_vec()];

    let quic_cfg = QuicClientConfig::try_from(tls_config)
        .map_err(|e| EngineError::Io(io::Error::other(format!("quic cfg: {e}"))))?;

    let mut client_cfg = quinn::ClientConfig::new(Arc::new(quic_cfg));
    let mut transport = quinn::TransportConfig::default();
    transport.max_idle_timeout(Some(std::time::Duration::from_secs(30).try_into().unwrap()));
    client_cfg.transport_config(Arc::new(transport));

    let bind_addr = "0.0.0.0:0".parse::<std::net::SocketAddr>().map_err(|e| {
        EngineError::Io(io::Error::other(format!("quic bind addr: {e}")))
    })?;

    let mut endpoint = quinn::Endpoint::client(bind_addr)
        .map_err(|e| EngineError::Io(io::Error::other(format!("quic endpoint: {e}"))))?;

    endpoint.set_default_client_config(client_cfg);

    let server_addr = format!("{server_name}:{port}").parse::<std::net::SocketAddr>().map_err(|e| {
        EngineError::Io(io::Error::other(format!("quic addr parse: {e}")))
    })?;

    let conn = endpoint
        .connect(server_addr, server_name)
        .map_err(|e| EngineError::Io(io::Error::other(format!("quic connect: {e}"))))?
        .await
        .map_err(|e| EngineError::Io(io::Error::other(format!("quic connection: {e}"))))?;

    let (send, recv) = conn
        .open_bi()
        .await
        .map_err(|e| EngineError::Io(io::Error::other(format!("quic open stream: {e}"))))?;

    Ok(QuicStream::new(send, recv))
}

// ── server (inbound) accept ──

#[cfg(feature = "inbound-vless")]
pub(crate) struct QuicInbound {
    endpoint: quinn::Endpoint,
}

#[cfg(feature = "inbound-vless")]
impl QuicInbound {
    pub(crate) async fn bind(
        listen_addr: &str,
        cert_path: &str,
        key_path: &str,
        base_dir: Option<&Path>,
    ) -> Result<Self, EngineError> {
        use std::fs::File;
        use std::io::BufReader;

        let cert_path = resolve_path(base_dir, cert_path);
        let key_path = resolve_path(base_dir, key_path);

        let cert_file = File::open(&cert_path).map_err(|e| {
            EngineError::Io(io::Error::other(format!(
                "quic cert file `{}`: {e}",
                cert_path.display()
            )))
        })?;
        let mut reader = BufReader::new(cert_file);
        let certs: Vec<rustls::pki_types::CertificateDer<'static>> =
            rustls_pemfile::certs(&mut reader)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| EngineError::Io(io::Error::other(format!("quic cert parse: {e}"))))?;

        let key_file = File::open(&key_path).map_err(|e| {
            EngineError::Io(io::Error::other(format!(
                "quic key file `{}`: {e}",
                key_path.display()
            )))
        })?;
        let mut reader = BufReader::new(key_file);
        let key = rustls_pemfile::private_key(&mut reader)
            .map_err(|e| EngineError::Io(io::Error::other(format!("quic key parse: {e}"))))?
            .ok_or_else(|| {
                EngineError::Io(io::Error::other("quic key file contains no private key"))
            })?;

        let mut server_cfg = quinn::ServerConfig::with_single_cert(certs, key)
            .map_err(|e| EngineError::Io(io::Error::other(format!("quic server cfg: {e}"))))?;

        let mut transport = quinn::TransportConfig::default();
        transport.max_idle_timeout(Some(std::time::Duration::from_secs(30).try_into().unwrap()));
        server_cfg.transport_config(Arc::new(transport));

        let bind_addr = listen_addr
            .parse::<std::net::SocketAddr>()
            .map_err(|e| EngineError::Io(io::Error::other(format!("quic bind addr: {e}"))))?;

        let endpoint = quinn::Endpoint::server(server_cfg, bind_addr)
            .map_err(|e| EngineError::Io(io::Error::other(format!("quic endpoint: {e}"))))?;

        Ok(Self { endpoint })
    }

    pub(crate) async fn accept(&self) -> Result<QuicStream, EngineError> {
        let conn = self
            .endpoint
            .accept()
            .await
            .ok_or_else(|| {
                EngineError::Io(io::Error::new(
                    io::ErrorKind::ConnectionAborted,
                    "quic endpoint closed",
                ))
            })?
            .await
            .map_err(|e| EngineError::Io(io::Error::other(format!("quic accept: {e}"))))?;

        let (send, recv) = conn
            .accept_bi()
            .await
            .map_err(|e| EngineError::Io(io::Error::other(format!("quic accept stream: {e}"))))?;

        Ok(QuicStream::new(send, recv))
    }
}

// ── SkipServerVerification for QUIC client ──

#[cfg(feature = "outbound-vless")]
#[derive(Debug)]
struct SkipServerVerification;

#[cfg(feature = "outbound-vless")]
impl SkipServerVerification {
    fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

#[cfg(feature = "outbound-vless")]
impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
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

// ── AsyncRead / AsyncWrite / AsyncSocket / ClientStream ──

impl AsyncRead for QuicStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.recv).poll_read(cx, buf)
    }
}

impl AsyncWrite for QuicStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.send).poll_write(cx, buf).map_err(|e| io::Error::other(e))
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.send).poll_flush(cx).map_err(|e| io::Error::other(e))
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.send).poll_shutdown(cx).map_err(|e| io::Error::other(e))
    }
}

impl AsyncSocket for QuicStream {
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

impl ClientStream for QuicStream {
    #[cfg(feature = "inbound-socks5")]
    fn local_addr(&self) -> io::Result<SocketAddr> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "QuicStream does not expose local_addr",
        ))
    }
}

fn resolve_path(base_dir: Option<&Path>, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        return path;
    }
    base_dir
        .map(|base_dir| base_dir.join(&path))
        .unwrap_or(path)
}
