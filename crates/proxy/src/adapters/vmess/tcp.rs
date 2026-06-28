use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::unreachable_leaf;
use crate::adapters::vmess::mux_pool::VmessMuxOpenRequest;
use crate::adapters::vmess::VmessAdapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::Proxy;
use crate::transport::{EstablishedTcpOutbound, MeteredStream, TcpOutboundFailure, TcpRelayStream};

fn vmess_tcp_config(id: &str, cipher: &str) -> Result<vmess::VmessTcpConnectConfig, EngineError> {
    vmess::VmessTcpConnectConfig::from_config(id, cipher).map_err(|error| {
        EngineError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, error))
    })
}

impl VmessAdapter {
    pub(super) async fn connect_tcp_impl(
        &self,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let ResolvedLeafOutbound::Vmess {
            tag,
            server,
            port,
            id,
            cipher,
            mux_concurrency,
            mux_idle_timeout_secs: _,
            tls,
            ws,
            grpc,
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf));
        };
        let config = vmess_tcp_config(id, cipher).map_err(|error| TcpOutboundFailure {
            stage: "connect_upstream_vmess",
            error,
            upstream_endpoint: Some(((*server).to_string(), *port)),
        })?;
        if let Some(max_concurrency) = mux_concurrency {
            return self
                .mux_pool
                .open_stream(VmessMuxOpenRequest {
                    proxy,
                    session,
                    server: (*server).to_owned(),
                    port: *port,
                    identity: vmess::VmessMuxIdentity::from_parts(
                        config.uuid(),
                        (*cipher).to_owned(),
                        config.cipher(),
                    ),
                    tls: *tls,
                    ws: *ws,
                    grpc: *grpc,
                    max_concurrency: *max_concurrency,
                })
                .await
                .map(|upstream| EstablishedTcpOutbound::Vmess {
                    tag: (*tag).to_string(),
                    server: (*server).to_string(),
                    port: *port,
                    upstream,
                })
                .map_err(|error| TcpOutboundFailure {
                    stage: "connect_upstream_vmess",
                    error,
                    upstream_endpoint: Some(((*server).to_string(), *port)),
                });
        }
        match connect_tcp(VmessTcpConnect {
            proxy,
            session,
            server,
            port: *port,
            config,
            tls: *tls,
            ws: *ws,
            grpc: *grpc,
        })
        .await
        {
            Ok(upstream) => Ok(EstablishedTcpOutbound::Vmess {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                upstream,
            }),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_upstream_vmess",
                error,
                upstream_endpoint: Some(((*server).to_string(), *port)),
            }),
        }
    }

    pub(super) async fn apply_relay_hop_impl(
        &self,
        stream: crate::transport::TcpRelayStream,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<crate::transport::TcpRelayStream, EngineError> {
        let ResolvedLeafOutbound::Vmess { id, cipher, .. } = leaf else {
            return Err(unreachable_leaf(self.name(), leaf).error);
        };
        let config = vmess_tcp_config(id, cipher)?;
        apply_tcp_hop(stream, session, config).await
    }
}

struct VmessTcpConnect<'a> {
    proxy: &'a Proxy,
    session: &'a Session,
    server: &'a str,
    port: u16,
    config: vmess::VmessTcpConnectConfig,
    tls: Option<&'a zero_config::ClientTlsConfig>,
    ws: Option<&'a zero_config::WebSocketConfig>,
    grpc: Option<&'a zero_config::GrpcConfig>,
}

async fn connect_tcp(request: VmessTcpConnect<'_>) -> Result<TcpRelayStream, EngineError> {
    let VmessTcpConnect {
        proxy,
        session,
        server,
        port,
        config,
        tls,
        ws,
        grpc,
    } = request;

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
        vmess::establish_tcp_outbound_session(&mut sock, session, &config.uuid(), config.cipher())
            .await?;
    proxy.record_session_outbound_traffic(session.id, sock.drain_traffic());
    Ok(TcpRelayStream::new(vmess::wrap_tcp_outbound_stream(
        sock.into_inner(),
        vmess_session,
    )?))
}

async fn apply_tcp_hop(
    stream: TcpRelayStream,
    session: &Session,
    config: vmess::VmessTcpConnectConfig,
) -> Result<TcpRelayStream, EngineError> {
    Ok(TcpRelayStream::new(
        vmess::establish_tcp_outbound_stream(stream, session, &config.uuid(), config.cipher())
            .await
            .map_err(|error| EngineError::Io(std::io::Error::other(error)))?,
    ))
}
