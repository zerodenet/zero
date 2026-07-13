use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use zero_config::{GrpcConfig, H2Config, HttpUpgradeConfig, SplitHttpConfig, WebSocketConfig};
use zero_engine::EngineError;
use zero_platform_tokio::{ClientStream, PrefixedSocket, TcpRelayStream, TokioSocket};

use crate::inbound_stack::{accept_inbound_stream_stack, InboundStreamStack};
use crate::{http_upgrade, split_http, tls};

pub(super) enum VlessInboundTransportResult {
    Stream {
        stream: VlessInboundTransportStream,
        sni: Option<String>,
    },
    FallbackReplay(vless::inbound::VlessFallbackReplay<TokioSocket>),
}

pub(super) enum VlessInboundTransportStream {
    Raw(TokioSocket),
    Tls(Box<tls::InboundTlsStream<PrefixedSocket>>),
    Reality(Box<vless::reality::RealityTlsStream<TokioSocket>>),
}

pub(super) async fn accept_vless_inbound_transport(
    stream: TokioSocket,
    tls_acceptor: Option<tls::TlsAcceptor>,
    reality: Option<vless::reality::VlessRealityServerProfile>,
    fallback_alpn: Option<String>,
) -> Result<VlessInboundTransportResult, EngineError> {
    match (tls_acceptor.as_ref(), reality.as_ref()) {
        (Some(acceptor), None) => {
            accept_vless_tls_inbound_transport(stream, acceptor, fallback_alpn).await
        }
        (None, Some(profile)) => Ok(VlessInboundTransportResult::Stream {
            stream: VlessInboundTransportStream::Reality(Box::new(
                profile.upgrade_server(stream).await?,
            )),
            sni: None,
        }),
        (None, None) => Ok(VlessInboundTransportResult::Stream {
            stream: VlessInboundTransportStream::Raw(stream),
            sni: None,
        }),
        (Some(_), Some(_)) => Err(EngineError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "vless inbound cannot set both tls and reality",
        ))),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn accept_vless_inbound_carrier(
    stream: VlessInboundTransportStream,
    sni: Option<String>,
    ws: Option<WebSocketConfig>,
    grpc: Option<GrpcConfig>,
    h2: Option<H2Config>,
    split_http: Option<SplitHttpConfig>,
    split_http_registry: Option<split_http::SplitHttpRegistry>,
    http_upgrade: Option<HttpUpgradeConfig>,
) -> Result<Option<(TcpRelayStream, Option<String>)>, EngineError> {
    if let Some(config) = split_http.as_ref() {
        let registry = split_http_registry.as_ref().ok_or_else(|| {
            EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "vless inbound: split-http registry is required",
            ))
        })?;
        return match split_http::accept_xhttp_inbound(stream, config, registry).await? {
            Some(stream) => Ok(Some((TcpRelayStream::new(stream), sni))),
            None => Ok(None),
        };
    }

    if let Some(config) = http_upgrade.as_ref() {
        let stream = http_upgrade::accept_http_upgrade(stream, config).await?;
        return Ok(Some((TcpRelayStream::new(stream), sni)));
    }

    let clear_sni = grpc.is_some();
    let stream = accept_inbound_stream_stack(
        stream,
        InboundStreamStack {
            ws: ws.as_ref(),
            grpc: grpc.as_ref(),
            h2: h2.as_ref(),
        },
        "vless inbound: ws, grpc, and h2 are mutually exclusive",
    )
    .await?;
    Ok(Some((stream, if clear_sni { None } else { sni })))
}

impl zero_traits::AsyncSocket for VlessInboundTransportStream {
    type Error = io::Error;

    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        match self {
            Self::Raw(stream) => zero_traits::AsyncSocket::read(stream, buf).await,
            Self::Tls(stream) => {
                <tls::InboundTlsStream<PrefixedSocket> as zero_traits::AsyncSocket>::read(
                    stream.as_mut(),
                    buf,
                )
                .await
            }
            Self::Reality(stream) => {
                <vless::reality::RealityTlsStream<TokioSocket> as zero_traits::AsyncSocket>::read(
                    stream.as_mut(),
                    buf,
                )
                .await
            }
        }
    }

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        match self {
            Self::Raw(stream) => stream.write_all(buf).await,
            Self::Tls(stream) => stream.write_all(buf).await,
            Self::Reality(stream) => stream.write_all(buf).await,
        }
    }

    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        match self {
            Self::Raw(stream) => stream.shutdown().await,
            Self::Tls(stream) => stream.shutdown().await,
            Self::Reality(stream) => stream.shutdown().await,
        }
    }
}

impl AsyncRead for VlessInboundTransportStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.as_mut().get_mut() {
            Self::Raw(stream) => Pin::new(stream).poll_read(cx, buf),
            Self::Tls(stream) => Pin::new(stream).poll_read(cx, buf),
            Self::Reality(stream) => Pin::new(stream).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for VlessInboundTransportStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.as_mut().get_mut() {
            Self::Raw(stream) => Pin::new(stream).poll_write(cx, buf),
            Self::Tls(stream) => Pin::new(stream).poll_write(cx, buf),
            Self::Reality(stream) => Pin::new(stream).poll_write(cx, buf),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.as_mut().get_mut() {
            Self::Raw(stream) => Pin::new(stream).poll_flush(cx),
            Self::Tls(stream) => Pin::new(stream).poll_flush(cx),
            Self::Reality(stream) => Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.as_mut().get_mut() {
            Self::Raw(stream) => Pin::new(stream).poll_shutdown(cx),
            Self::Tls(stream) => Pin::new(stream).poll_shutdown(cx),
            Self::Reality(stream) => Pin::new(stream).poll_shutdown(cx),
        }
    }
}

impl ClientStream for VlessInboundTransportStream {
    fn local_addr(&self) -> io::Result<std::net::SocketAddr> {
        match self {
            Self::Raw(stream) => stream.local_addr(),
            Self::Tls(stream) => stream.local_addr(),
            Self::Reality(_stream) => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "reality inbound local_addr not available",
            )),
        }
    }

    fn peer_addr(&self) -> io::Result<std::net::SocketAddr> {
        match self {
            Self::Raw(stream) => stream.peer_addr(),
            Self::Tls(stream) => stream.peer_addr(),
            Self::Reality(_stream) => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "reality inbound peer_addr not available",
            )),
        }
    }
}

async fn accept_vless_tls_inbound_transport(
    mut stream: TokioSocket,
    tls_acceptor: &tls::TlsAcceptor,
    fallback_alpn: Option<String>,
) -> Result<VlessInboundTransportResult, EngineError> {
    let hello = tls::peek_client_hello(&mut stream).await.ok().flatten();

    if let Some(hello) = hello {
        let tls::InboundClientHello {
            sni,
            alpn,
            consumed,
        } = hello;
        if let Some(expected_alpn) = fallback_alpn.as_deref() {
            match vless::inbound::fallback_replay_for_alpns(
                Some(expected_alpn),
                alpn.iter().map(|alpn| alpn.as_str()),
                stream,
                consumed,
            )
            .into_transport_parts()
            {
                Ok(fallback_replay) => {
                    return Ok(VlessInboundTransportResult::FallbackReplay(fallback_replay));
                }
                Err((stream, replay_head)) => {
                    let tls_stream = tls_acceptor
                        .accept(PrefixedSocket::from_prefix(stream, replay_head))
                        .await
                        .map_err(|error| EngineError::Io(io::Error::other(error)))?;
                    return Ok(VlessInboundTransportResult::Stream {
                        stream: VlessInboundTransportStream::Tls(Box::new(
                            tls::InboundTlsStream::new_generic(tls_stream),
                        )),
                        sni,
                    });
                }
            }
        }

        let tls_stream = tls_acceptor
            .accept(PrefixedSocket::from_prefix(stream, consumed))
            .await
            .map_err(|error| EngineError::Io(io::Error::other(error)))?;
        return Ok(VlessInboundTransportResult::Stream {
            stream: VlessInboundTransportStream::Tls(Box::new(tls::InboundTlsStream::new_generic(
                tls_stream,
            ))),
            sni,
        });
    }

    let tls_stream = tls_acceptor
        .accept(PrefixedSocket::from_prefix(stream, Vec::new()))
        .await
        .map_err(|error| EngineError::Io(io::Error::other(error)))?;
    Ok(VlessInboundTransportResult::Stream {
        stream: VlessInboundTransportStream::Tls(Box::new(tls::InboundTlsStream::new_generic(
            tls_stream,
        ))),
        sni: None,
    })
}
