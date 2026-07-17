use zero_core::Session;

use crate::inventory::PreparedTcpRelayChain;
use crate::protocol_registry::TcpRuntimeServices;
#[cfg(feature = "udp-runtime")]
use crate::transport::RelayCarrier;
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure};

pub(crate) async fn dispatch_prepared_tcp_relay_chain(
    services: TcpRuntimeServices,
    session: &Session,
    prepared: PreparedTcpRelayChain<'_>,
) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
    let (stream, final_hop) = execute_relay_prefix(services.clone(), prepared).await?;
    let stream = dispatch_prepared_tcp_relay_hop(services, stream, session, final_hop)
        .await
        .map_err(|error| TcpOutboundFailure {
            stage: "relay_last",
            error,
            upstream_endpoint: None,
        })?;

    Ok(EstablishedTcpOutbound::relay(stream))
}

pub(crate) async fn dispatch_prepared_tcp_relay_hop(
    services: TcpRuntimeServices,
    stream: crate::transport::TcpRelayStream,
    session: &Session,
    prepared: crate::inventory::PreparedTcpRelayHop<'_>,
) -> Result<crate::transport::TcpRelayStream, zero_engine::EngineError> {
    prepared.operation.execute(services, stream, session).await
}

#[cfg(feature = "udp-runtime")]

pub(crate) async fn dispatch_prepared_tcp_relay_carrier(
    services: TcpRuntimeServices,
    prepared: PreparedTcpRelayChain<'_>,
) -> Result<RelayCarrier, TcpOutboundFailure> {
    let (stream, final_hop) = execute_relay_prefix(services, prepared).await?;
    let (server, port) = final_hop.upstream();
    Ok(RelayCarrier {
        stream,
        server,
        port,
    })
}

async fn execute_relay_prefix<'a>(
    services: TcpRuntimeServices,
    prepared: PreparedTcpRelayChain<'a>,
) -> Result<
    (
        crate::transport::TcpRelayStream,
        crate::inventory::PreparedTcpRelayHop<'a>,
    ),
    TcpOutboundFailure,
> {
    let mut relay_hops = prepared.relay_hops.into_iter();
    let mut current_prepared = relay_hops
        .next()
        .expect("relay chain must have at least one prepared hop");
    let mut session_for_next = current_prepared.next_session();

    let outbound = super::candidate::dispatch_prepared_tcp_candidate(
        services.clone(),
        &session_for_next,
        prepared.first,
    )
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
        stream = dispatch_prepared_tcp_relay_hop(
            services.clone(),
            stream,
            &session_for_next,
            current_prepared,
        )
        .await
        .map_err(|error| TcpOutboundFailure {
            stage: "relay_hop",
            error,
            upstream_endpoint: None,
        })?;
        current_prepared = next_prepared;
    }

    Ok((stream, current_prepared))
}
