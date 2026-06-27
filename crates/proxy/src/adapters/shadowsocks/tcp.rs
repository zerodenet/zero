use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::unreachable_leaf;
use crate::adapters::shadowsocks::ShadowsocksAdapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::Proxy;
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure};

fn tcp_config(
    cipher: &str,
    password: &str,
    stage: &'static str,
) -> Result<shadowsocks::ShadowsocksTcpConnectConfig, EngineError> {
    shadowsocks::ShadowsocksTcpConnectConfig::from_config(cipher, password).map_err(|error| {
        EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{stage}: {error}"),
        ))
    })
}

impl ShadowsocksAdapter {
    pub(super) async fn connect_tcp_impl(
        &self,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let ResolvedLeafOutbound::Shadowsocks {
            tag,
            server,
            port,
            password,
            cipher,
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf));
        };
        let config =
            tcp_config(cipher, password, "invalid shadowsocks tcp config").map_err(|error| {
                TcpOutboundFailure {
                    stage: "connect_upstream_shadowsocks",
                    error,
                    upstream_endpoint: Some(((*server).to_string(), *port)),
                }
            })?;
        match crate::outbound::shadowsocks::connect_tcp(
            crate::outbound::shadowsocks::ShadowsocksTcpConnectRequest {
                proxy,
                session,
                server,
                port: *port,
                config,
            },
        )
        .await
        {
            Ok(upstream) => Ok(EstablishedTcpOutbound::Shadowsocks {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                upstream,
            }),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_upstream_shadowsocks",
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
        let ResolvedLeafOutbound::Shadowsocks {
            password, cipher, ..
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf).error);
        };
        let config = tcp_config(cipher, password, "invalid shadowsocks tcp relay config")?;
        crate::outbound::shadowsocks::apply_tcp_hop(stream, session, config).await
    }
}
