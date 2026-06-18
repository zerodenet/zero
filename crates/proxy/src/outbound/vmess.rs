//! VMess outbound — TCP connect.
//!
//! TCP outbound connect ([`connect_tcp`]) moved here from `runtime/upstream.rs`
//! so the runtime dispatches via the `ProtocolAdapter` trait. UDP management
//! lives in `crate::runtime::vmess_udp`.

use zero_config::{ClientTlsConfig, GrpcConfig, WebSocketConfig};
use zero_core::Session;
use zero_engine::EngineError;
use zero_traits::TcpSessionProtocol;

use crate::runtime::Proxy;
use crate::transport::{MeteredStream, TcpRelayStream};

/// Establish a VMess TCP upstream: resolve the MUX fast path, dial the server,
/// apply the transport stack (gRPC > WS > TLS > raw), run the VMess AEAD
/// session handshake.
///
/// Moved from `runtime/upstream.rs`. The runtime dispatches via the adapter
/// trait instead of a per-protocol `connect_via_*` method.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn connect_tcp(
    proxy: &Proxy,
    session: &Session,
    server: &str,
    port: u16,
    id: &str,
    cipher: &str,
    mux_concurrency: Option<u32>,
    mux_idle_timeout_secs: Option<u64>,
    tls: Option<&ClientTlsConfig>,
    ws: Option<&WebSocketConfig>,
    grpc: Option<&GrpcConfig>,
) -> Result<TcpRelayStream, EngineError> {
    use vmess::{parse_uuid, VmessCipher, VmessOutbound};

    let uuid = parse_uuid(id)
        .map_err(|e| EngineError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, e)))?;
    let vmess_cipher = VmessCipher::from_name(cipher).ok_or_else(|| {
        EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("vmess unknown cipher: {cipher}"),
        ))
    })?;

    if let Some(max_concurrency) = mux_concurrency {
        return proxy
            .vmess_mux_pool
            .open_stream(
                proxy,
                session,
                server.to_owned(),
                port,
                uuid,
                cipher.to_owned(),
                tls,
                ws,
                grpc,
                max_concurrency,
            )
            .await;
    }
    let _ = mux_idle_timeout_secs;

    let socket = proxy
        .protocols
        .direct_outbound
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
        <VmessOutbound as TcpSessionProtocol<vmess::VmessTcpSessionTarget>>::establish_tcp_session(
            &VmessOutbound,
            &mut sock,
            &vmess::VmessTcpSessionTarget {
                session,
                uuid: &uuid,
                cipher: vmess_cipher,
            },
        )
        .await?;
    proxy.record_session_outbound_traffic(session.id, sock.drain_traffic());
    Ok(TcpRelayStream::new(vmess::VmessAeadStream::outbound(
        sock.into_inner(),
        vmess_session,
    )?))
}

/// Apply a VMess AEAD session handshake over an existing stream (relay hop).
/// Unlike [`connect_tcp`] this does not dial.
pub(crate) async fn apply_tcp_hop(
    mut stream: TcpRelayStream,
    session: &Session,
    id: &str,
    cipher: &str,
) -> Result<TcpRelayStream, EngineError> {
    use vmess::{parse_uuid, VmessCipher, VmessOutbound};

    let uuid = parse_uuid(id)
        .map_err(|e| EngineError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, e)))?;
    let vmess_cipher = VmessCipher::from_name(cipher).ok_or_else(|| {
        EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("vmess unknown cipher: {cipher}"),
        ))
    })?;
    let vmess_session =
        <VmessOutbound as TcpSessionProtocol<vmess::VmessTcpSessionTarget>>::establish_tcp_session(
            &VmessOutbound,
            &mut stream,
            &vmess::VmessTcpSessionTarget {
                session,
                uuid: &uuid,
                cipher: vmess_cipher,
            },
        )
        .await
        .map_err(|e| EngineError::Io(std::io::Error::other(e)))?;
    Ok(TcpRelayStream::new(vmess::VmessAeadStream::outbound(
        stream,
        vmess_session,
    )?))
}
