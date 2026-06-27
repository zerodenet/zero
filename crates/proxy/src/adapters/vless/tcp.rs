use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::unreachable_leaf;
use crate::adapters::vless::mux_pool::VlessMuxOpenRequest;
use crate::adapters::vless::VlessAdapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::Proxy;
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure};

fn vless_tcp_config(
    id: &str,
    flow: Option<&str>,
) -> Result<vless::VlessTcpConnectConfig, EngineError> {
    vless::VlessTcpConnectConfig::from_config(id, flow).map_err(|error| {
        EngineError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, error))
    })
}

impl VlessAdapter {
    pub(super) async fn connect_tcp_impl(
        &self,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let ResolvedLeafOutbound::Vless {
            tag,
            server,
            port,
            id,
            flow,
            mux_concurrency,
            mux_idle_timeout_secs,
            tls,
            reality,
            ws,
            grpc,
            h2,
            http_upgrade,
            split_http,
            quic,
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf));
        };
        let config = vless_tcp_config(id, *flow).map_err(|error| TcpOutboundFailure {
            stage: "connect_upstream_vless",
            error,
            upstream_endpoint: Some(((*server).to_string(), *port)),
        })?;
        if *flow == Some("xtls-rprx-vision") {
            return self
                .mux_pool
                .open_stream(VlessMuxOpenRequest {
                    proxy,
                    session: Some(session),
                    server,
                    port: *port,
                    identity: vless::mux_pool::MuxIdentity::from_uuid(config.id()),
                    tls: *tls,
                    reality: *reality,
                    max_concurrency: mux_concurrency.unwrap_or(8),
                })
                .await
                .map(|upstream| EstablishedTcpOutbound::Vless {
                    tag: (*tag).to_string(),
                    server: (*server).to_string(),
                    port: *port,
                    upstream,
                })
                .map_err(|error| TcpOutboundFailure {
                    stage: "connect_upstream_vless",
                    error,
                    upstream_endpoint: Some(((*server).to_string(), *port)),
                });
        }
        match crate::outbound::vless::connect_tcp(crate::outbound::vless::VlessTcpConnectRequest {
            proxy,
            session,
            server,
            port: *port,
            config,
            mux_concurrency: *mux_concurrency,
            mux_idle_timeout_secs: *mux_idle_timeout_secs,
            tls: *tls,
            reality: *reality,
            ws: *ws,
            grpc: *grpc,
            h2: *h2,
            http_upgrade: *http_upgrade,
            quic: *quic,
            split_http: *split_http,
        })
        .await
        {
            Ok(upstream) => Ok(EstablishedTcpOutbound::Vless {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                upstream,
            }),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_upstream_vless",
                error,
                upstream_endpoint: Some(((*server).to_string(), *port)),
            }),
        }
    }

    pub(super) async fn apply_relay_hop_impl(
        &self,
        proxy: &Proxy,
        stream: crate::transport::TcpRelayStream,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<crate::transport::TcpRelayStream, EngineError> {
        let ResolvedLeafOutbound::Vless { id, flow, .. } = leaf else {
            return Err(unreachable_leaf(self.name(), leaf).error);
        };
        let config = vless_tcp_config(id, *flow)?;
        crate::outbound::vless::apply_tcp_hop(proxy, stream, session, config).await
    }
}
