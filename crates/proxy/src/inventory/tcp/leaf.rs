use zero_engine::EngineError;

use super::super::ProtocolInventory;
use crate::inventory::ClaimedInventoryLeaf;
use crate::protocol_registry::{OutboundAdapterContext, TcpRuntimeServices};
use crate::runtime::path::TcpPathCategory;
use crate::runtime::tcp_dispatch::operation::{
    PreparedTcpConnectOperation, PreparedTcpRelayOperation,
};
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure, TcpRelayStream};

pub(crate) struct PreparedTcpCandidate<'a> {
    health_tag: Option<String>,
    execution: PreparedTcpCandidateExecution<'a>,
}

enum PreparedTcpCandidateExecution<'a> {
    Block { tag: String },
    Connect(Box<dyn PreparedTcpConnectOperation + 'a>),
}

impl PreparedTcpCandidate<'_> {
    pub(crate) fn health_tag(&self) -> Option<&str> {
        self.health_tag.as_deref()
    }

    pub(crate) async fn execute(
        self,
        services: TcpRuntimeServices,
        session: &zero_core::Session,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        match self.execution {
            PreparedTcpCandidateExecution::Block { tag } => Ok(EstablishedTcpOutbound::block(tag)),
            PreparedTcpCandidateExecution::Connect(operation) => {
                operation.execute(services, session).await
            }
        }
    }
}

pub(crate) struct PreparedTcpRelayHop<'a> {
    server: String,
    port: u16,
    operation: Box<dyn PreparedTcpRelayOperation + 'a>,
}

impl PreparedTcpRelayHop<'_> {
    pub(crate) fn next_session(&self) -> zero_core::Session {
        zero_core::Session::new(
            0,
            zero_core::Address::Domain(self.server.clone()),
            self.port,
            zero_core::Network::Tcp,
            zero_core::ProtocolType::Unknown,
        )
    }

    #[cfg(any(
        feature = "socks5",
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    pub(crate) fn upstream(&self) -> (String, u16) {
        (self.server.clone(), self.port)
    }

    pub(crate) async fn execute(
        self,
        services: TcpRuntimeServices,
        stream: TcpRelayStream,
        session: &zero_core::Session,
    ) -> Result<TcpRelayStream, EngineError> {
        self.operation.execute(services, stream, session).await
    }
}

impl ProtocolInventory {
    pub(super) fn prepare_claimed_tcp_candidate<'a>(
        &self,
        ctx: OutboundAdapterContext,
        claimed: &ClaimedInventoryLeaf<'a>,
    ) -> Result<PreparedTcpCandidate<'a>, TcpOutboundFailure> {
        let runtime = claimed.runtime();
        let health_tag = health_tag(runtime).map(ToOwned::to_owned);
        let execution = if matches!(runtime.tcp_path, TcpPathCategory::Block) {
            PreparedTcpCandidateExecution::Block {
                tag: runtime.kernel_tag.unwrap_or("block").to_owned(),
            }
        } else {
            let operation = claimed.prepare_tcp_connect(ctx.source_dir())?;
            PreparedTcpCandidateExecution::Connect(operation)
        };
        Ok(PreparedTcpCandidate {
            health_tag,
            execution,
        })
    }

    pub(crate) fn prepare_tcp_candidate<'a>(
        &self,
        ctx: OutboundAdapterContext,
        leaf: &'a zero_engine::ResolvedLeafOutbound<'a>,
    ) -> Result<PreparedTcpCandidate<'a>, TcpOutboundFailure> {
        let claimed = self
            .claim_outbound_leaf(leaf)
            .map_err(|error| TcpOutboundFailure {
                stage: "outbound_leaf_runtime",
                error,
                upstream_endpoint: None,
            })?;
        self.prepare_claimed_tcp_candidate(ctx, &claimed)
    }

    pub(super) fn prepare_claimed_tcp_relay_hop<'a>(
        &self,
        ctx: OutboundAdapterContext,
        claimed: &ClaimedInventoryLeaf<'a>,
    ) -> Result<PreparedTcpRelayHop<'a>, EngineError> {
        let (server, port, operation) = claimed.prepare_tcp_relay_hop(ctx.source_dir())?;
        Ok(PreparedTcpRelayHop {
            server: server.to_owned(),
            port,
            operation,
        })
    }

    #[cfg(test)]
    pub(crate) fn prepare_tcp_relay_hop<'a>(
        &self,
        ctx: OutboundAdapterContext,
        leaf: &'a zero_engine::ResolvedLeafOutbound<'a>,
    ) -> Result<PreparedTcpRelayHop<'a>, EngineError> {
        let claimed = self.claim_outbound_leaf(leaf)?;
        self.prepare_claimed_tcp_relay_hop(ctx, &claimed)
    }
}

fn health_tag(runtime: crate::protocol_registry::OutboundLeafRuntime<'_>) -> Option<&str> {
    match runtime.tcp_path {
        TcpPathCategory::Direct | TcpPathCategory::Block => None,
        #[cfg(any(feature = "socks5", feature = "vless", feature = "trojan"))]
        TcpPathCategory::Tunnel => runtime.health_tag,
        #[cfg(any(feature = "shadowsocks", feature = "vmess", feature = "mieru"))]
        TcpPathCategory::Session => runtime.health_tag,
        #[cfg(feature = "hysteria2")]
        TcpPathCategory::TransportSession => runtime.health_tag,
    }
}
