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
use zero_traits::{AsyncSocket, ClientTlsProfile};

use zero_engine::EngineError;
use zero_platform_tokio::ClientStream;
use zero_platform_tokio::TcpRelayStream;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct InboundClientHello {
    pub sni: Option<String>,
    pub alpn: Vec<String>,
    pub consumed: Vec<u8>,
}

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

    // Look up server fingerprint preset for cipher suite preference control
    let fingerprint = tls
        .server_fingerprint
        .as_deref()
        .and_then(|name| {
            let fp = crate::fingerprint::lookup_fingerprint(name);
            if fp.is_none() {
                tracing::warn!(fingerprint = %name, "unknown tls server fingerprint preset, using defaults");
            }
            fp
        });

    let config_builder = if let Some(ref fp) = fingerprint {
        let provider = Arc::new(crate::fingerprint::build_provider(fp));
        tracing::debug!(
            fingerprint = %tls.server_fingerprint.as_deref().unwrap_or(""),
            cipher_count = fp.cipher_suites.len(),
            "tls server fingerprint applied"
        );
        rustls::ServerConfig::builder_with_provider(provider)
            .with_protocol_versions(&[&rustls::version::TLS13, &rustls::version::TLS12])
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?
    } else {
        rustls::ServerConfig::builder()
    };

    let mut config = config_builder
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

pub async fn accept_tls_inbound(
    stream: TokioSocket,
    acceptor: &TlsAcceptor,
) -> Result<InboundTlsStream<TokioSocket>, EngineError> {
    let tls = acceptor
        .accept(stream)
        .await
        .map_err(|error| EngineError::Io(io::Error::other(error)))?;
    Ok(InboundTlsStream::new_generic(tls))
}

pub async fn peek_client_hello<R>(reader: &mut R) -> io::Result<Option<InboundClientHello>>
where
    R: AsyncRead + Unpin,
{
    let mut consumed = Vec::with_capacity(512);

    let mut record_hdr = [0u8; 5];
    reader.read_exact(&mut record_hdr).await?;
    if record_hdr[0] != 0x16 {
        return Ok(None);
    }
    consumed.extend_from_slice(&record_hdr);

    let mut handshake_hdr = [0u8; 4];
    reader.read_exact(&mut handshake_hdr).await?;
    if handshake_hdr[0] != 0x01 {
        return Ok(None);
    }
    consumed.extend_from_slice(&handshake_hdr);

    let mut fixed = [0u8; 35];
    read_exact_client_hello(reader, &mut consumed, &mut fixed).await?;
    let session_id_len = fixed[34] as usize;
    skip_exact_client_hello(reader, &mut consumed, session_id_len).await?;

    let mut cipher_suites_len = [0u8; 2];
    read_exact_client_hello(reader, &mut consumed, &mut cipher_suites_len).await?;
    skip_exact_client_hello(
        reader,
        &mut consumed,
        u16::from_be_bytes(cipher_suites_len) as usize,
    )
    .await?;

    let mut compression_methods_len = [0u8; 1];
    read_exact_client_hello(reader, &mut consumed, &mut compression_methods_len).await?;
    skip_exact_client_hello(reader, &mut consumed, compression_methods_len[0] as usize).await?;

    let mut extensions_len = [0u8; 2];
    match reader.read_exact(&mut extensions_len).await {
        Ok(_) => consumed.extend_from_slice(&extensions_len),
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
            return Ok(Some(InboundClientHello {
                consumed,
                ..Default::default()
            }));
        }
        Err(error) => return Err(error),
    }

    let extensions_len = u16::from_be_bytes(extensions_len).min(8192) as usize;
    let mut extensions = vec![0u8; extensions_len];
    read_exact_client_hello(reader, &mut consumed, &mut extensions).await?;

    Ok(Some(parse_client_hello_extensions(&extensions, consumed)))
}

pub async fn connect_tls_upstream_with_profile<P>(
    socket: TokioSocket,
    tls: &P,
    base_dir: Option<&Path>,
    default_server_name: &str,
) -> Result<TcpRelayStream, EngineError>
where
    P: ClientTlsProfile + ?Sized,
{
    let server_name = tls.server_name().unwrap_or(default_server_name).to_owned();

    let mut roots = RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    if let Some(path) = tls.ca_cert_path() {
        for cert in load_certs(&resolve_path(base_dir, path))? {
            roots
                .add(cert)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
        }
    }

    // Look up fingerprint preset for builder-time configuration
    let fingerprint = tls.client_fingerprint().and_then(|name| {
        let fp = crate::fingerprint::lookup_fingerprint(name);
        if fp.is_none() {
            tracing::warn!(fingerprint = %name, "unknown tls fingerprint preset, using defaults");
        }
        fp
    });

    // Build with optional fingerprint via custom CryptoProvider
    let config_base = if let Some(ref fp) = fingerprint {
        let provider = Arc::new(crate::fingerprint::build_provider(fp));
        tracing::debug!(
            fingerprint = %tls.client_fingerprint().unwrap_or(""),
            cipher_count = fp.cipher_suites.len(),
            "tls fingerprint applied"
        );
        ClientConfig::builder_with_provider(provider)
            .with_protocol_versions(&[&rustls::version::TLS13, &rustls::version::TLS12])
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?
    } else {
        ClientConfig::builder()
    };

    let mut config = if tls.insecure() {
        config_base
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(InsecureCertVerifier))
            .with_no_client_auth()
    } else {
        config_base
            .with_root_certificates(roots)
            .with_no_client_auth()
    };

    if tls.disable_sni() {
        config.enable_sni = false;
    }

    // ALPN: use explicit config if provided, otherwise use fingerprint-suggested ALPN
    if !tls.alpn().is_empty() {
        config.alpn_protocols = tls
            .alpn()
            .iter()
            .map(|proto| proto.as_bytes().to_vec())
            .collect();
    } else if fingerprint.is_some() && config.alpn_protocols.is_empty() {
        config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    }

    let server_name_str = server_name.clone();

    // Use custom TLS 1.3 handshake when fingerprint is specified
    if let Some(ref fp) = fingerprint {
        tracing::debug!(
            sni = %server_name_str,
            fingerprint = %tls.client_fingerprint().unwrap_or(""),
            "connecting via custom TLS 1.3 handshake"
        );
        return connect_tls13_upstream(socket, &server_name_str, fp).await;
    }

    let connector = TlsConnector::from(Arc::new(config));
    let server_name = rustls::pki_types::ServerName::try_from(server_name.as_str())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid tls server_name"))?
        .to_owned();

    let peer_addr = socket.peer_addr().ok();
    tracing::debug!(
        sni = %server_name_str,
        peer = ?peer_addr,
        insecure = tls.insecure(),
        alpn = ?tls.alpn(),
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

    Ok(TcpRelayStream::new(stream))
}

pub async fn connect_tls_upstream(
    socket: TokioSocket,
    tls: &ClientTlsConfig,
    base_dir: Option<&Path>,
    default_server_name: &str,
) -> Result<TcpRelayStream, EngineError> {
    connect_tls_upstream_with_profile(socket, tls, base_dir, default_server_name).await
}

pub async fn connect_tls_stream_with_profile<S, P>(
    stream: S,
    tls: &P,
    base_dir: Option<&Path>,
    default_server_name: &str,
) -> Result<TcpRelayStream, EngineError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    P: ClientTlsProfile + ?Sized,
{
    let server_name = tls.server_name().unwrap_or(default_server_name).to_owned();

    let mut roots = RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    if let Some(path) = tls.ca_cert_path() {
        for cert in load_certs(&resolve_path(base_dir, path))? {
            roots
                .add(cert)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
        }
    }

    // Route fingerprint TLS through the generic ztls async handshake path
    if let Some(fp) = tls
        .client_fingerprint()
        .and_then(crate::fingerprint::lookup_fingerprint)
    {
        return connect_tls13_stream(stream, &server_name, &fp).await;
    }

    let mut config = if tls.insecure() {
        ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(InsecureCertVerifier))
            .with_no_client_auth()
    } else {
        ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth()
    };

    if tls.disable_sni() {
        config.enable_sni = false;
    }

    if !tls.alpn().is_empty() {
        config.alpn_protocols = tls
            .alpn()
            .iter()
            .map(|proto| proto.as_bytes().to_vec())
            .collect();
    }

    let connector = TlsConnector::from(Arc::new(config));
    let server_name = rustls::pki_types::ServerName::try_from(server_name.as_str())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid tls server_name"))?
        .to_owned();

    let stream = connector.connect(server_name, stream).await?;

    Ok(TcpRelayStream::new(stream))
}

pub async fn connect_tls_stream<S>(
    stream: S,
    tls: &ClientTlsConfig,
    base_dir: Option<&Path>,
    default_server_name: &str,
) -> Result<TcpRelayStream, EngineError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
{
    connect_tls_stream_with_profile(stream, tls, base_dir, default_server_name).await
}

/// Connect over a generic AsyncRead+AsyncWrite stream using our custom
/// TLS 1.3 stack with the requested fingerprint's cipher suites and
/// ClientHello control. Suitable for relay-stream TLS (where the stream
/// is not a concrete TcpStream).
async fn connect_tls13_stream<S>(
    stream: S,
    server_name: &str,
    fp: &crate::fingerprint::TlsFingerprint,
) -> Result<TcpRelayStream, EngineError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
{
    let cipher_suites: Vec<ztls::cipher::CipherSuite> = fp
        .cipher_suites
        .iter()
        .filter_map(|s| rustls_to_ztls_suite(s.suite().as_str()?))
        .collect();

    let suites = if cipher_suites.is_empty() {
        ztls::cipher::DEFAULT_CIPHER_SUITES.to_vec()
    } else {
        cipher_suites
    };

    let config = ztls::handshake::Tls13Config {
        server_name: server_name.to_owned(),
        cipher_suites: suites,
        alpn_protocols: vec!["h2".to_owned(), "http/1.1".to_owned()],
        handshake_timeout_ms: 15_000,
    };

    let tls_stream = ztls::stream::Tls13Stream::connect_async(stream, config)
        .await
        .map_err(|e| {
            EngineError::Io(io::Error::other(format!(
                "custom TLS handshake over relay stream: {e}"
            )))
        })?;

    Ok(TcpRelayStream::new(tls_stream))
}

/// Connect using our custom TLS 1.3 stack with the requested
/// fingerprint's cipher suites and ClientHello control.
/// Fast path for fresh sockets (uses spawn_blocking + into_std).
async fn connect_tls13_upstream(
    socket: TokioSocket,
    server_name: &str,
    fp: &crate::fingerprint::TlsFingerprint,
) -> Result<TcpRelayStream, EngineError> {
    // Convert rustls cipher suite names to ztls CipherSuites.
    // rustls uses "TLS13_AES_128_GCM_SHA256", ztls uses "TLS_AES_128_GCM_SHA256".
    let cipher_suites: Vec<ztls::cipher::CipherSuite> = fp
        .cipher_suites
        .iter()
        .filter_map(|s| rustls_to_ztls_suite(s.suite().as_str()?))
        .collect();

    let suites = if cipher_suites.is_empty() {
        ztls::cipher::DEFAULT_CIPHER_SUITES.to_vec()
    } else {
        cipher_suites
    };

    let config = ztls::handshake::Tls13Config {
        server_name: server_name.to_owned(),
        cipher_suites: suites,
        alpn_protocols: vec!["h2".to_owned(), "http/1.1".to_owned()],
        handshake_timeout_ms: 15_000,
    };

    let inner = socket.into_inner();
    let tls_stream = ztls::stream::Tls13Stream::connect(inner, config)
        .await
        .map_err(|e| EngineError::Io(io::Error::other(format!("custom TLS handshake: {e}"))))?;

    Ok(TcpRelayStream::new(tls_stream))
}

/// Map a rustls cipher suite name to the equivalent ztls CipherSuite.
fn rustls_to_ztls_suite(name: &str) -> Option<ztls::cipher::CipherSuite> {
    let ztls_name = match name {
        "TLS13_AES_128_GCM_SHA256" => "TLS_AES_128_GCM_SHA256",
        "TLS13_AES_256_GCM_SHA384" => "TLS_AES_256_GCM_SHA384",
        "TLS13_CHACHA20_POLY1305_SHA256" => "TLS_CHACHA20_POLY1305_SHA256",
        // TLS 1.2 suites that also appear in some fingerprints
        "TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256" => "TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256",
        "TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256" => "TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256",
        _ => return None,
    };
    ztls::cipher::CipherSuite::from_name(ztls_name)
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

fn parse_client_hello_extensions(ext_data: &[u8], consumed: Vec<u8>) -> InboundClientHello {
    let mut sni = None;
    let mut alpn = Vec::new();
    let mut offset = 0;

    while offset + 4 <= ext_data.len() {
        let ext_type = u16::from_be_bytes([ext_data[offset], ext_data[offset + 1]]);
        let ext_len = u16::from_be_bytes([ext_data[offset + 2], ext_data[offset + 3]]) as usize;
        offset += 4;
        if offset + ext_len > ext_data.len() {
            break;
        }
        let ext_bytes = &ext_data[offset..offset + ext_len];
        match ext_type {
            0x0000 => {
                if ext_bytes.len() >= 5 && ext_bytes[2] == 0x00 {
                    let name_len = u16::from_be_bytes([ext_bytes[3], ext_bytes[4]]) as usize;
                    if 5 + name_len <= ext_bytes.len() {
                        if let Ok(name) = std::str::from_utf8(&ext_bytes[5..5 + name_len]) {
                            sni = Some(name.to_owned());
                        }
                    }
                }
            }
            0x0010 if ext_bytes.len() >= 4 => {
                let list_len = u16::from_be_bytes([ext_bytes[2], ext_bytes[3]]) as usize;
                let mut pos = 4;
                while pos < ext_bytes.len() && pos < 4 + list_len {
                    let proto_len = ext_bytes[pos] as usize;
                    pos += 1;
                    if pos + proto_len <= ext_bytes.len() {
                        if let Ok(proto) = std::str::from_utf8(&ext_bytes[pos..pos + proto_len]) {
                            alpn.push(proto.to_owned());
                        }
                        pos += proto_len;
                    } else {
                        break;
                    }
                }
            }
            _ => {}
        }
        offset += ext_len;
    }

    InboundClientHello {
        sni,
        alpn,
        consumed,
    }
}

async fn read_exact_client_hello<R>(
    reader: &mut R,
    consumed: &mut Vec<u8>,
    dest: &mut [u8],
) -> io::Result<()>
where
    R: AsyncRead + Unpin,
{
    reader.read_exact(dest).await?;
    consumed.extend_from_slice(dest);
    Ok(())
}

async fn skip_exact_client_hello<R>(
    reader: &mut R,
    consumed: &mut Vec<u8>,
    len: usize,
) -> io::Result<()>
where
    R: AsyncRead + Unpin,
{
    let mut chunk = vec![0u8; len.min(256)];
    let mut remaining = len;
    while remaining > 0 {
        let to_read = remaining.min(chunk.len());
        reader.read_exact(&mut chunk[..to_read]).await?;
        consumed.extend_from_slice(&chunk[..to_read]);
        remaining -= to_read;
    }
    Ok(())
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

    fn read<'a>(
        &'a mut self,
        buf: &'a mut [u8],
    ) -> impl core::future::Future<Output = Result<usize, Self::Error>> + Send + 'a {
        async move { AsyncReadExt::read(&mut self.inner, buf).await }
    }

    fn write_all<'a>(
        &'a mut self,
        buf: &'a [u8],
    ) -> impl core::future::Future<Output = Result<(), Self::Error>> + Send + 'a {
        async move { AsyncWriteExt::write_all(&mut self.inner, buf).await }
    }

    fn shutdown<'a>(
        &'a mut self,
    ) -> impl core::future::Future<Output = Result<(), Self::Error>> + Send + 'a {
        async move { AsyncWriteExt::shutdown(&mut self.inner).await }
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
