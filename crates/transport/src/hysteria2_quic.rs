use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use zero_config::InboundProtocolConfig;
use zero_engine::EngineError;
use zero_traits::AsyncSocket;

/// Bidirectional QUIC stream wrapper used by Hysteria2 proxy glue.
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

    fn read<'a>(
        &'a mut self,
        buf: &'a mut [u8],
    ) -> impl core::future::Future<Output = Result<usize, Self::Error>> + Send + 'a {
        async move { AsyncReadExt::read(self, buf).await }
    }

    fn write_all<'a>(
        &'a mut self,
        buf: &'a [u8],
    ) -> impl core::future::Future<Output = Result<(), Self::Error>> + Send + 'a {
        async move {
            AsyncWriteExt::write_all(self, buf).await?;
            AsyncWriteExt::flush(self).await
        }
    }

    fn shutdown<'a>(
        &'a mut self,
    ) -> impl core::future::Future<Output = Result<(), Self::Error>> + Send + 'a {
        async move { AsyncWriteExt::shutdown(self).await }
    }
}

#[derive(Debug, Clone)]
pub struct OwnedHysteria2InboundBindPlan {
    cert_path: String,
    key_path: String,
    source_dir: Option<PathBuf>,
}

impl OwnedHysteria2InboundBindPlan {
    pub fn from_config_ref(
        source_dir: Option<&Path>,
        cert_path: Option<&str>,
        key_path: Option<&str>,
    ) -> Self {
        Self {
            cert_path: cert_path.unwrap_or("certs/fullchain.pem").to_owned(),
            key_path: key_path.unwrap_or("certs/privkey.pem").to_owned(),
            source_dir: source_dir.map(PathBuf::from),
        }
    }

    pub fn from_protocol_config(
        protocol: &InboundProtocolConfig,
        source_dir: Option<&Path>,
    ) -> Result<Self, EngineError> {
        match protocol {
            InboundProtocolConfig::Hysteria2 {
                cert_path,
                key_path,
                ..
            } => Ok(Self::from_config_ref(
                source_dir,
                cert_path.as_deref(),
                key_path.as_deref(),
            )),
            _ => Err(EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "hysteria2 inbound bind plan received non-hysteria2 inbound config",
            ))),
        }
    }

    pub async fn bind(&self, listen_addr: &str) -> Result<crate::quic::QuicInbound, EngineError> {
        crate::quic::QuicInbound::bind(
            listen_addr,
            &self.cert_path,
            &self.key_path,
            self.source_dir.as_deref(),
        )
        .await
    }
}

#[async_trait::async_trait]
impl crate::inbound_route::ProtocolInboundBindPlan for OwnedHysteria2InboundBindPlan {
    fn from_protocol_config(
        protocol: &InboundProtocolConfig,
        source_dir: Option<&Path>,
    ) -> Result<Self, EngineError> {
        Self::from_protocol_config(protocol, source_dir)
    }

    async fn bind(
        &self,
        listen_addr: &str,
    ) -> Result<crate::inbound_route::TransportInboundBindTarget, EngineError> {
        Ok(crate::inbound_route::TransportInboundBindTarget::Quic(
            OwnedHysteria2InboundBindPlan::bind(self, listen_addr).await?,
        ))
    }
}

pub struct QuicConnectionOptions<'a> {
    pub server: &'a str,
    pub port: u16,
    pub alpn: Vec<Vec<u8>>,
    pub quic_profile: Hysteria2QuicProfile,
    pub datagram_receive_buffer_size: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hysteria2QuicProfile {
    client_fingerprint: Option<String>,
}

impl Hysteria2QuicProfile {
    pub fn from_parts(client_fingerprint: Option<&str>) -> Self {
        Self {
            client_fingerprint: client_fingerprint.map(ToOwned::to_owned),
        }
    }

    fn client_fingerprint(&self) -> Option<&str> {
        self.client_fingerprint.as_deref()
    }
}

pub async fn open_quic_connection(
    options: QuicConnectionOptions<'_>,
) -> Result<quinn::Connection, EngineError> {
    let config_base = if let Some(fp_name) = options.quic_profile.client_fingerprint() {
        if let Some(preset) = crate::fingerprint::lookup_fingerprint(fp_name) {
            let provider = std::sync::Arc::new(crate::fingerprint::build_provider(&preset));
            tracing::debug!(
                fingerprint = %fp_name,
                "quic tls fingerprint applied"
            );
            rustls::ClientConfig::builder_with_provider(provider)
                .with_protocol_versions(&[&rustls::version::TLS13, &rustls::version::TLS12])
                .map_err(|error| {
                    EngineError::Io(io::Error::other(format!("quic tls protocol: {error}")))
                })?
        } else {
            tracing::warn!(
                fingerprint = %fp_name,
                "unknown quic tls fingerprint, using defaults"
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
    tls_config.alpn_protocols = options.alpn;

    let quic_cfg = quinn::crypto::rustls::QuicClientConfig::try_from(tls_config)
        .map_err(|error| EngineError::Io(io::Error::other(format!("quic tls cfg: {error}"))))?;

    let mut client_cfg = quinn::ClientConfig::new(Arc::new(quic_cfg));
    let mut transport = quinn::TransportConfig::default();
    transport.max_idle_timeout(Some(std::time::Duration::from_secs(30).try_into().unwrap()));
    transport.datagram_receive_buffer_size(options.datagram_receive_buffer_size);
    client_cfg.transport_config(Arc::new(transport));

    let bind_addr: std::net::SocketAddr = "0.0.0.0:0"
        .parse()
        .map_err(|error| EngineError::Io(io::Error::other(format!("quic bind addr: {error}"))))?;
    let socket = std::net::UdpSocket::bind(bind_addr)
        .map_err(|error| EngineError::Io(io::Error::other(format!("quic bind socket: {error}"))))?;
    let mut endpoint = quinn::Endpoint::new(
        quinn::EndpointConfig::default(),
        None,
        socket,
        Arc::new(quinn::TokioRuntime),
    )
    .map_err(|error| EngineError::Io(io::Error::other(format!("quic endpoint: {error}"))))?;
    endpoint.set_default_client_config(client_cfg);

    let server_addr = format!("{}:{}", options.server, options.port)
        .parse::<std::net::SocketAddr>()
        .map_err(|error| EngineError::Io(io::Error::other(format!("quic addr: {error}"))))?;

    endpoint
        .connect(server_addr, options.server)
        .map_err(|error| EngineError::Io(io::Error::other(format!("quic connect: {error}"))))?
        .await
        .map_err(|error| EngineError::Io(io::Error::other(format!("quic connection: {error}"))))
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
