use std::fs::File;
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
#[cfg(feature = "inbound-vless")]
use std::pin::Pin;
use std::sync::Arc;
#[cfg(feature = "inbound-vless")]
use std::task::{Context, Poll};

#[cfg(feature = "inbound-vless")]
use rustls::pki_types::PrivateKeyDer;
#[cfg(feature = "outbound-vless")]
use rustls::{ClientConfig, RootCertStore};
#[cfg(feature = "inbound-vless")]
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
#[cfg(feature = "inbound-vless")]
use tokio::net::TcpStream;
#[cfg(feature = "inbound-vless")]
use tokio_rustls::server::TlsStream;
#[cfg(feature = "inbound-vless")]
use tokio_rustls::TlsAcceptor;
#[cfg(feature = "outbound-vless")]
use tokio_rustls::TlsConnector;
#[cfg(feature = "outbound-vless")]
use zero_config::ClientTlsConfig;
#[cfg(feature = "inbound-vless")]
use zero_config::TlsConfig;
#[cfg(feature = "outbound-vless")]
use zero_platform_tokio::TokioSocket;
#[cfg(feature = "inbound-vless")]
use zero_traits::AsyncSocket;

#[cfg(feature = "inbound-vless")]
use super::stream::ClientStream;
#[cfg(feature = "outbound-vless")]
use super::stream::TcpRelayStream;
use zero_engine::EngineError;

#[cfg(feature = "inbound-vless")]
pub(crate) fn build_tls_acceptor(
    tls: &TlsConfig,
    base_dir: Option<&Path>,
) -> Result<TlsAcceptor, EngineError> {
    let certs = load_certs(&resolve_path(base_dir, &tls.cert_path))?;
    let key = load_private_key(&resolve_path(base_dir, &tls.key_path))?;
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    Ok(TlsAcceptor::from(Arc::new(config)))
}

#[cfg(feature = "outbound-vless")]
pub(crate) async fn connect_tls_upstream(
    socket: TokioSocket,
    tls: &ClientTlsConfig,
    base_dir: Option<&Path>,
    default_server_name: &str,
) -> Result<TcpRelayStream, EngineError> {
    let server_name = tls
        .server_name
        .as_deref()
        .unwrap_or(default_server_name)
        .to_owned();
    let server_name = rustls::pki_types::ServerName::try_from(server_name.as_str())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid tls server_name"))?
        .to_owned();
    let mut roots = RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    if let Some(path) = &tls.ca_cert_path {
        for cert in load_certs(&resolve_path(base_dir, path))? {
            roots
                .add(cert)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
        }
    }

    let config = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let connector = TlsConnector::from(Arc::new(config));
    let stream = connector.connect(server_name, socket.into_inner()).await?;

    Ok(TcpRelayStream::Tls(Box::new(stream)))
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

#[cfg(feature = "inbound-vless")]
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

#[cfg(feature = "inbound-vless")]
pub(crate) struct InboundTlsStream {
    inner: TlsStream<TcpStream>,
}

#[cfg(feature = "inbound-vless")]
impl InboundTlsStream {
    pub(crate) fn new(inner: TlsStream<TcpStream>) -> Self {
        Self { inner }
    }
}

#[cfg(feature = "inbound-vless")]
impl ClientStream for InboundTlsStream {
    #[cfg(feature = "inbound-socks5")]
    fn local_addr(&self) -> io::Result<std::net::SocketAddr> {
        self.inner.get_ref().0.local_addr()
    }
}

#[cfg(feature = "inbound-vless")]
impl AsyncSocket for InboundTlsStream {
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

#[cfg(feature = "inbound-vless")]
impl AsyncRead for InboundTlsStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

#[cfg(feature = "inbound-vless")]
impl AsyncWrite for InboundTlsStream {
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
