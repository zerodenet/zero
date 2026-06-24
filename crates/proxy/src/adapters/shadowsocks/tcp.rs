use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::unreachable_leaf;
use crate::adapters::shadowsocks::ShadowsocksAdapter;
use crate::protocol_adapter::ProtocolAdapter;
use crate::runtime::Proxy;
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure};

fn parse_shadowsocks_cipher(cipher: &str) -> Result<shadowsocks::CipherKind, EngineError> {
    shadowsocks::CipherKind::from_str(cipher).ok_or_else(|| {
        EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("unknown shadowsocks cipher: {cipher}"),
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
        let cipher_kind = parse_shadowsocks_cipher(cipher).map_err(|error| TcpOutboundFailure {
            stage: "connect_upstream_shadowsocks",
            error,
            upstream_endpoint: Some(((*server).to_string(), *port)),
        })?;
        match crate::outbound::shadowsocks::connect_tcp(
            crate::outbound::shadowsocks::ShadowsocksTcpConnectRequest {
                proxy,
                session,
                server,
                port: *port,
                password,
                cipher: cipher_kind,
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
        let cipher = parse_shadowsocks_cipher(cipher)?;
        crate::outbound::shadowsocks::apply_tcp_hop(stream, session, password, cipher).await
    }
}
