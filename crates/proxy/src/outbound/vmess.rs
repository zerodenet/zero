//! VMess outbound — TCP connect.
//!
//! TCP outbound connect ([`connect_tcp`]) moved here from `runtime/upstream.rs`
//! so the runtime dispatches via registered TCP outbound capabilities. UDP management
//! glue lives under the VMess adapter UDP module.

use zero_core::Session;
use zero_engine::EngineError;

use crate::runtime::Proxy;
use crate::transport::{MeteredStream, TcpRelayStream};

/// Establish a VMess TCP upstream: resolve the MUX fast path, dial the server,
/// apply the transport stack (gRPC > WS > TLS > raw), run the VMess AEAD
/// session handshake.
///
/// Moved from `runtime/upstream.rs`. The runtime dispatches via the adapter
/// trait instead of a per-protocol `connect_via_*` method.
pub(crate) async fn connect_tcp(
    request: VmessTcpConnectRequest<'_>,
) -> Result<TcpRelayStream, EngineError> {
    let VmessTcpConnectRequest {
        proxy,
        session,
        server,
        port,
        uuid,
        cipher,
        mux_concurrency,
        mux_idle_timeout_secs,
        tls,
        ws,
        grpc,
    } = request;

    let _ = mux_concurrency;
    let _ = mux_idle_timeout_secs;

    let socket = proxy
        .protocols
        .direct_connector()
        .connect_host(server, port, proxy.resolver.as_ref())
        .await?;

    let stream = crate::transport::build_vmess_outbound_transport(
        crate::transport::VmessOutboundTransportRequest {
            socket,
            options: crate::transport::VmessTransportOptions {
                tls,
                ws,
                grpc,
                source_dir: proxy.config.source_dir(),
            },
            server,
            port,
        },
    )
    .await?;

    let mut sock = MeteredStream::new(stream);
    let vmess_session =
        vmess::establish_tcp_outbound_session(&mut sock, session, &uuid, cipher).await?;
    proxy.record_session_outbound_traffic(session.id, sock.drain_traffic());
    Ok(TcpRelayStream::new(vmess::wrap_tcp_outbound_stream(
        sock.into_inner(),
        vmess_session,
    )?))
}

pub(crate) struct VmessTcpConnectRequest<'a> {
    pub proxy: &'a Proxy,
    pub session: &'a Session,
    pub server: &'a str,
    pub port: u16,
    pub uuid: [u8; 16],
    pub cipher: vmess::VmessCipher,
    pub mux_concurrency: Option<u32>,
    pub mux_idle_timeout_secs: Option<u64>,
    pub tls: Option<&'a zero_config::ClientTlsConfig>,
    pub ws: Option<&'a zero_config::WebSocketConfig>,
    pub grpc: Option<&'a zero_config::GrpcConfig>,
}

/// Apply a VMess AEAD session handshake over an existing stream (relay hop).
/// Unlike [`connect_tcp`] this does not dial.
pub(crate) async fn apply_tcp_hop(
    stream: TcpRelayStream,
    session: &Session,
    uuid: [u8; 16],
    cipher: vmess::VmessCipher,
) -> Result<TcpRelayStream, EngineError> {
    Ok(TcpRelayStream::new(
        vmess::establish_tcp_outbound_stream(stream, session, &uuid, cipher)
            .await
            .map_err(|e| EngineError::Io(std::io::Error::other(e)))?,
    ))
}
