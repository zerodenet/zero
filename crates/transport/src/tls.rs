use std::fs::File;
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use rustls::pki_types::PrivateKeyDer;
use rustls::{ClientConfig, RootCertStore};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::TcpStream;
use tokio_rustls::server::TlsStream;
pub use tokio_rustls::TlsAcceptor;
use tokio_rustls::TlsConnector;
use zero_config::ClientTlsConfig;
use zero_config::TlsConfig;
use zero_platform_tokio::TokioSocket;
use zero_traits::AsyncSocket;

use zero_engine::EngineError;
use zero_platform_tokio::ClientStream;

#[derive(Debug)]
struct InsecureCertVerifier;

impl rustls::client::danger::ServerCertVerifier for InsecureCertVerifier {
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
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

pub fn build_tls_acceptor(
    tls: &TlsConfig,
    base_dir: Option<&Path>,
) -> Result<TlsAcceptor, EngineError> {
    let certs = load_certs(&resolve_path(base_dir, &tls.cert_path))?;
    let key = load_private_key(&resolve_path(base_dir, &tls.key_path))?;
    let mut config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    if !tls.alpn.is_empty() {
        config.alpn_protocols = tls
            .alpn
            .iter()
            .map(|proto| proto.as_bytes().to_vec())
            .collect();
    }

    Ok(TlsAcceptor::from(Arc::new(config)))
}

pub async fn connect_tls_upstream(
    socket: TokioSocket,
    tls: &ClientTlsConfig,
    base_dir: Option<&Path>,
    default_server_name: &str,
) -> Result<tokio_rustls::client::TlsStream<TcpStream>, EngineError> {
    let server_name = tls
        .server_name
        .as_deref()
        .unwrap_or(default_server_name)
        .to_owned();

    let mut roots = RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    if let Some(path) = &tls.ca_cert_path {
        for cert in load_certs(&resolve_path(base_dir, path))? {
            roots
                .add(cert)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
        }
    }

    let mut config = if tls.insecure {
        ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(InsecureCertVerifier))
            .with_no_client_auth()
    } else {
        ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth()
    };

    if tls.disable_sni {
        config.enable_sni = false;
    }

    if !tls.alpn.is_empty() {
        config.alpn_protocols = tls
            .alpn
            .iter()
            .map(|proto| proto.as_bytes().to_vec())
            .collect();
    }

    let server_name_str = server_name.clone();
    let connector = TlsConnector::from(Arc::new(config));
    let server_name = rustls::pki_types::ServerName::try_from(server_name.as_str())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid tls server_name"))?
        .to_owned();

    let peer_addr = socket.peer_addr().ok();
    tracing::debug!(
        sni = %server_name_str,
        peer = ?peer_addr,
        insecure = tls.insecure,
        alpn = ?tls.alpn,
        "tls connecting"
    );

    let stream = connector
        .connect(server_name, socket.into_inner())
        .await
        .map_err(|e| {
            tracing::warn!(
                error = %e,
                sni = %server_name_str,
                peer = ?peer_addr,
                "tls handshake failed"
            );
            e
        })?;

    Ok(stream)
}

fn load_certs(path: &Path) -> io::Result<Vec<rustls::pki_types::CertificateDer<'static>>> {
    let file = File::open(path).map_err(|source| {
        io::Error::new(
            source.kind(),
            format!(
                "failed to read tls certificate `{}`: {source}",
                path.display()
            ),
        )
    })?;
    let mut reader = BufReader::new(file);
    let certs = rustls_pemfile::certs(&mut reader).collect::<Result<Vec<_>, _>>()?;
    if certs.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "tls certificate `{}` contains no certificates",
                path.display()
            ),
        ));
    }

    Ok(certs)
}

fn load_private_key(path: &Path) -> io::Result<PrivateKeyDer<'static>> {
    let file = File::open(path).map_err(|source| {
        io::Error::new(
            source.kind(),
            format!(
                "failed to read tls private key `{}`: {source}",
                path.display()
            ),
        )
    })?;
    let mut reader = BufReader::new(file);
    rustls_pemfile::private_key(&mut reader)?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "tls private key `{}` contains no private key",
                path.display()
            ),
        )
    })
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

pub struct InboundTlsStream<IO = TcpStream> {
    inner: TlsStream<IO>,
}

impl InboundTlsStream {
    pub fn new(inner: TlsStream<TcpStream>) -> Self {
        Self { inner }
    }
}

impl<IO> InboundTlsStream<IO> {
    pub fn new_generic(inner: TlsStream<IO>) -> Self {
        Self { inner }
    }
}

impl<IO> ClientStream for InboundTlsStream<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    fn local_addr(&self) -> io::Result<std::net::SocketAddr> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "not available"))
    }
}

impl<IO> AsyncSocket for InboundTlsStream<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    type Error = io::Error;

    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        AsyncReadExt::read(&mut self.inner, buf).await
    }

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        AsyncWriteExt::write_all(&mut self.inner, buf).await
    }

    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        AsyncWriteExt::shutdown(&mut self.inner).await
    }
}

impl<IO> AsyncRead for InboundTlsStream<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<IO> AsyncWrite for InboundTlsStream<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}
