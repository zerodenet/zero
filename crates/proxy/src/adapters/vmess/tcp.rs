use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::unreachable_leaf;
use crate::adapters::vmess::mux_pool::VmessMuxOpenRequest;
use crate::adapters::vmess::VmessAdapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::Proxy;
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure};

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
            mux_idle_timeout_secs,
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
        match crate::outbound::vmess::connect_tcp(crate::outbound::vmess::VmessTcpConnectRequest {
            proxy,
            session,
            server,
            port: *port,
            config,
            mux_concurrency: *mux_concurrency,
            mux_idle_timeout_secs: *mux_idle_timeout_secs,
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
        crate::outbound::vmess::apply_tcp_hop(stream, session, config).await
    }
}
