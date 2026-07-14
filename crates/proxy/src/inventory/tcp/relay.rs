use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use super::super::ProtocolInventory;
use super::dispatch_prepared_tcp_candidate;
use super::{PreparedTcpCandidate, PreparedTcpRelayHop};
use crate::protocol_registry::{OutboundAdapterContext, TcpRuntimeServices};
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use crate::transport::RelayCarrier;
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure};

pub(crate) struct PreparedTcpRelayChain<'a> {
    first: PreparedTcpCandidate<'a>,
    relay_hops: Vec<PreparedTcpRelayHop<'a>>,
}

impl PreparedTcpRelayChain<'_> {
    pub(crate) async fn execute(
        self,
        services: TcpRuntimeServices,
        session: &Session,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let mut relay_hops = self.relay_hops.into_iter();
        let mut current_prepared = relay_hops
            .next()
            .expect("relay chain must have at least one prepared hop");
        let mut session_for_next = current_prepared.next_session();

        let outbound =
            dispatch_prepared_tcp_candidate(services.clone(), &session_for_next, self.first)
                .await?;
        let mut stream = outbound
            .into_relay_stream()
            .map_err(|error| TcpOutboundFailure {
                stage: "relay_first_hop",
                error,
                upstream_endpoint: None,
            })?;

        for next_prepared in relay_hops {
            session_for_next = next_prepared.next_session();
            stream = current_prepared
                .execute(services.clone(), stream, &session_for_next)
                .await
                .map_err(|error| TcpOutboundFailure {
                    stage: "relay_hop",
                    error,
                    upstream_endpoint: None,
                })?;
            current_prepared = next_prepared;
        }

        let stream = current_prepared
            .execute(services.clone(), stream, session)
            .await
            .map_err(|error| TcpOutboundFailure {
                stage: "relay_last",
                error,
                upstream_endpoint: None,
            })?;

        Ok(EstablishedTcpOutbound::relay(stream))
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
    pub(crate) async fn into_relay_carrier(
        self,
        services: TcpRuntimeServices,
    ) -> Result<RelayCarrier, TcpOutboundFailure> {
        let mut relay_hops = self.relay_hops.into_iter();
        let mut current_prepared = relay_hops
            .next()
            .expect("relay chain must have at least one prepared hop");
        let mut session_for_next = current_prepared.next_session();

        let outbound =
            dispatch_prepared_tcp_candidate(services.clone(), &session_for_next, self.first)
                .await?;
        let mut stream = outbound
            .into_relay_stream()
            .map_err(|error| TcpOutboundFailure {
                stage: "relay_first_hop",
                error,
                upstream_endpoint: None,
            })?;

        for next_prepared in relay_hops {
            session_for_next = next_prepared.next_session();
            stream = current_prepared
                .execute(services.clone(), stream, &session_for_next)
                .await
                .map_err(|error| TcpOutboundFailure {
                    stage: "relay_hop",
                    error,
                    upstream_endpoint: None,
                })?;
            current_prepared = next_prepared;
        }

        let (server, port) = current_prepared.upstream();
        Ok(RelayCarrier {
            stream,
            server,
            port,
        })
    }
}

impl ProtocolInventory {
    pub(crate) fn prepare_tcp_relay_chain<'a>(
        &self,
        ctx: OutboundAdapterContext,
        chain: &'a [ResolvedLeafOutbound<'a>],
    ) -> Result<PreparedTcpRelayChain<'a>, TcpOutboundFailure> {
        let first = chain
            .first()
            .expect("relay chain must have at least 2 hops");
        let relay_hops = &chain[1..];
        let _second = relay_hops
            .first()
            .expect("relay chain must have at least 2 hops");

        let first_prepared = self.prepare_tcp_candidate(ctx.clone(), first)?;
        let mut prepared_hops = Vec::with_capacity(relay_hops.len());
        for next_hop in relay_hops {
            let prepared = self
                .prepare_tcp_relay_hop(ctx.clone(), next_hop)
                .map_err(|error| TcpOutboundFailure {
                    stage: "relay_prepare",
                    error,
                    upstream_endpoint: None,
                })?;
            prepared_hops.push(prepared);
        }

        Ok(PreparedTcpRelayChain {
            first: first_prepared,
            relay_hops: prepared_hops,
        })
    }
}

pub(crate) async fn dispatch_prepared_tcp_relay_chain(
    services: TcpRuntimeServices,
    session: &Session,
    prepared: PreparedTcpRelayChain<'_>,
) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
    prepared.execute(services, session).await
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
pub(crate) async fn dispatch_prepared_tcp_relay_carrier(
    services: TcpRuntimeServices,
    prepared: PreparedTcpRelayChain<'_>,
) -> Result<RelayCarrier, TcpOutboundFailure> {
    prepared.into_relay_carrier(services).await
}
