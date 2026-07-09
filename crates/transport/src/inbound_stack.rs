use std::io;
use std::path::Path;

use tokio::io::{AsyncRead, AsyncWrite};
use zero_config::{GrpcConfig, H2Config, TlsConfig, WebSocketConfig};
use zero_engine::EngineError;
use zero_platform_tokio::{TcpRelayStream, TokioSocket};

#[cfg(feature = "h2")]
use crate::h2;
use crate::{grpc, ws};

#[derive(Clone, Copy)]
pub struct InboundStreamStack<'a> {
    pub ws: Option<&'a WebSocketConfig>,
    pub grpc: Option<&'a GrpcConfig>,
    pub h2: Option<&'a H2Config>,
}

pub fn build_optional_tls_acceptor(
    source_dir: Option<&Path>,
    tls: Option<&TlsConfig>,
) -> Result<Option<crate::tls::TlsAcceptor>, EngineError> {
    tls.map(|tls| crate::tls::build_tls_acceptor(tls, source_dir))
        .transpose()
}

pub fn build_required_tls_acceptor(
    source_dir: Option<&Path>,
    tls: Option<&TlsConfig>,
    missing_message: &'static str,
) -> Result<crate::tls::TlsAcceptor, EngineError> {
    build_optional_tls_acceptor(source_dir, tls)?.ok_or_else(|| {
        EngineError::Io(io::Error::new(io::ErrorKind::InvalidInput, missing_message))
    })
}

pub async fn accept_tls_inbound_stream(
    socket: TokioSocket,
    tls_acceptor: &crate::tls::TlsAcceptor,
) -> Result<crate::tls::InboundTlsStream<TokioSocket>, EngineError> {
    crate::tls::accept_tls_inbound(socket, tls_acceptor).await
}

pub async fn accept_tls_inbound_stream_stack(
    socket: TokioSocket,
    tls_acceptor: &crate::tls::TlsAcceptor,
    stack: InboundStreamStack<'_>,
    invalid_message: &'static str,
) -> Result<TcpRelayStream, EngineError> {
    let tls = accept_tls_inbound_stream(socket, tls_acceptor).await?;
    accept_inbound_stream_stack(tls, stack, invalid_message).await
}

pub async fn accept_inbound_stream_stack<S>(
    stream: S,
    stack: InboundStreamStack<'_>,
    invalid_message: &'static str,
) -> Result<TcpRelayStream, EngineError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
{
    let InboundStreamStack {
        ws: ws_config,
        grpc: grpc_config,
        h2: h2_config,
    } = stack;

    #[cfg(not(feature = "h2"))]
    if h2_config.is_some() {
        return invalid_inbound_stack(invalid_message);
    }

    match (ws_config, grpc_config, h2_config) {
        (Some(config), None, None) => Ok(TcpRelayStream::new(
            ws::accept_ws(stream, &config.path).await?,
        )),
        (None, Some(config), None) => Ok(TcpRelayStream::new(
            grpc::accept_grpc(stream, &config.service_names).await?,
        )),
        #[cfg(feature = "h2")]
        (None, None, Some(config)) => Ok(TcpRelayStream::new(h2::accept_h2(stream, config).await?)),
        (None, None, None) => Ok(TcpRelayStream::new(stream)),
        _ => invalid_inbound_stack(invalid_message),
    }
}

fn invalid_inbound_stack(invalid_message: &'static str) -> Result<TcpRelayStream, EngineError> {
    Err(EngineError::Io(io::Error::new(
        io::ErrorKind::InvalidInput,
        invalid_message,
    )))
}
