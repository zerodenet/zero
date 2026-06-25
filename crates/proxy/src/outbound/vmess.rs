//! VMess outbound — TCP connect.
//!
//! TCP outbound connect ([`connect_tcp`]) moved here from `runtime/upstream.rs`
//! so the runtime dispatches via the `ProtocolAdapter` trait. UDP management
//! lives in `crate::protocol_runtime::vmess_udp`.

use zero_config::{ClientTlsConfig, GrpcConfig, WebSocketConfig};
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
        cipher_name,
        cipher,
        mux_concurrency,
        mux_idle_timeout_secs,
        tls,
        ws,
        grpc,
    } = request;

    if let Some(max_concurrency) = mux_concurrency {
        return proxy
            .vmess_mux_pool
            .open_stream(
                crate::protocol_runtime::vmess_mux_pool::model::VmessMuxOpenRequest {
                    proxy,
                    session,
                    server: server.to_owned(),
                    port,
                    id: uuid,
                    cipher_name: cipher_name.to_owned(),
                    cipher,
                    tls,
                    ws,
                    grpc,
                    max_concurrency,
                },
            )
            .await;
    }
    let _ = mux_idle_timeout_secs;

    let socket = proxy
        .protocols
        .direct_connector()
        .connect_host(server, port, proxy.resolver.as_ref())
        .await?;

    // Transport stack: gRPC > WS > TLS > raw
    let stream: TcpRelayStream = match (grpc, ws, tls) {
        (Some(grpc_cfg), None, Some(tls_cfg)) => {
            let tls_stream = zero_transport::tls::connect_tls_upstream(
                socket,
                tls_cfg,
                proxy.config.source_dir(),
                server,
            )
            .await?;
            TcpRelayStream::new(
                zero_transport::grpc::connect_grpc(tls_stream, &grpc_cfg.service_names).await?,
            )
        }
        (Some(grpc_cfg), None, None) => TcpRelayStream::new(
            zero_transport::grpc::connect_grpc(socket, &grpc_cfg.service_names).await?,
        ),
        (None, Some(ws_cfg), Some(tls_cfg)) => {
            let tls_stream = zero_transport::tls::connect_tls_upstream(
                socket,
                tls_cfg,
                proxy.config.source_dir(),
                server,
            )
            .await?;
            TcpRelayStream::new(
                zero_transport::ws::connect_ws(tls_stream, ws_cfg, server, port).await?,
            )
        }
        (None, Some(ws_cfg), None) => {
            TcpRelayStream::new(zero_transport::ws::connect_ws(socket, ws_cfg, server, port).await?)
        }
        (None, None, Some(tls_cfg)) => {
            let tls_stream = zero_transport::tls::connect_tls_upstream(
                socket,
                tls_cfg,
                proxy.config.source_dir(),
                server,
            )
            .await?;
            TcpRelayStream::new(tls_stream)
        }
        (None, None, None) => TcpRelayStream::new(socket),
        _ => {
            return Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "vmess: ws and grpc are mutually exclusive",
            )))
        }
    };

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
    pub cipher_name: &'a str,
    pub cipher: vmess::VmessCipher,
    pub mux_concurrency: Option<u32>,
    pub mux_idle_timeout_secs: Option<u64>,
    pub tls: Option<&'a ClientTlsConfig>,
    pub ws: Option<&'a WebSocketConfig>,
    pub grpc: Option<&'a GrpcConfig>,
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
