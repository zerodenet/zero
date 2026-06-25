use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::unreachable_leaf;
use crate::adapters::trojan::TrojanAdapter;
use crate::protocol_adapter::ProtocolSupportCapability;
use crate::runtime::Proxy;
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure};

impl TrojanAdapter {
    pub(super) async fn connect_tcp_impl(
        &self,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let ResolvedLeafOutbound::Trojan {
            tag,
            server,
            port,
            password,
            sni,
            insecure,
            client_fingerprint,
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf));
        };
        match crate::outbound::trojan::connect_tcp(
            crate::outbound::trojan::TrojanTcpConnectRequest {
                proxy,
                session,
                server,
                port: *port,
                password,
                sni: *sni,
                insecure: *insecure,
                client_fingerprint: *client_fingerprint,
            },
        )
        .await
        {
            Ok(upstream) => Ok(EstablishedTcpOutbound::Trojan {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                upstream,
            }),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_upstream_trojan",
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
        let ResolvedLeafOutbound::Trojan { password, .. } = leaf else {
            return Err(unreachable_leaf(self.name(), leaf).error);
        };
        crate::outbound::trojan::apply_tcp_hop(proxy, stream, session, password).await
    }
}
