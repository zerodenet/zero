use std::pin::Pin;

use zero_engine::EngineError;

use super::{InboundConnectionContext, PreparedInboundListenerOperation};
use crate::protocol_registry::BoundInbound;
use crate::runtime::route_runtime::{InboundListenerRuntime, InboundRouteRuntime};

pub(crate) struct AuthenticatedQuicInboundListenerOperation<P> {
    pub(crate) protocol_name: &'static str,
    pub(crate) profile: P,
}

impl<P> PreparedInboundListenerOperation for AuthenticatedQuicInboundListenerOperation<P>
where
    P: zero_transport::inbound_quic::AuthenticatedQuicInboundProfile,
{
    fn execute(
        self: Box<Self>,
        runtime: InboundListenerRuntime,
        bound: BoundInbound,
        shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), EngineError>> + Send + 'static>> {
        Box::pin(async move {
            let listener = match bound {
                BoundInbound::Quic(listener) => listener,
                BoundInbound::Tcp(_) => {
                    return Err(EngineError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "authenticated QUIC inbound received a TCP listener",
                    )))
                }
            };
            let profile = self.profile;
            let protocol_name = self.protocol_name;
            crate::runtime::listener_loop::run_quic_listener_loop(
                crate::runtime::listener_loop::QuicListenerLoopRequest {
                    runtime_factory: runtime.route_factory(),
                    protocol_name,
                    listener,
                    shutdown,
                    handler: move |runtime, connection| {
                        let profile = profile.clone();
                        async move {
                            if let Err(error) = run_authenticated_quic_connection(
                                profile,
                                runtime,
                                connection,
                            )
                            .await
                            {
                                tracing::error!(%error, protocol = protocol_name, "inbound QUIC connection failed");
                            }
                        }
                    },
                },
            )
            .await
        })
    }
}

async fn run_authenticated_quic_connection<P>(
    profile: P,
    runtime: InboundRouteRuntime,
    connection: quinn::Connection,
) -> Result<(), EngineError>
where
    P: zero_transport::inbound_quic::AuthenticatedQuicInboundProfile,
{
    use zero_transport::inbound_quic::AuthenticatedQuicInboundConnection;

    let connection = profile.accept_authenticated_connection(connection).await?;
    let mut tasks = tokio::task::JoinSet::new();
    let udp_source = connection.datagram_source();
    let udp_relay = connection.udp_relay();
    let udp_runtime = runtime.udp_runtime();
    let udp_tag = runtime.inbound_tag().to_owned();
    tasks.spawn(async move {
        crate::runtime::datagram_udp::run_protocol_datagram_udp_relay(
            udp_runtime,
            udp_source,
            udp_relay,
            &udp_tag,
            false,
        )
        .await
    });

    loop {
        tokio::select! {
            accepted = connection.accept_next_tcp_stream() => {
                let Some((session, stream)) = accepted? else {
                    break;
                };
                let context = InboundConnectionContext::new(runtime.clone());
                let response = connection.response_protocol();
                tasks.spawn(async move {
                    context.serve_with_client_response(session, stream, response).await
                });
            }
            result = tasks.join_next(), if !tasks.is_empty() => {
                match result {
                    Some(Ok(Ok(()))) => {}
                    Some(Ok(Err(error))) => tracing::warn!(%error, "inbound QUIC stream task failed"),
                    Some(Err(error)) if !error.is_cancelled() => {
                        tracing::error!(%error, "inbound QUIC stream task panicked");
                    }
                    Some(Err(_)) | None => {}
                }
            }
        }
    }

    tasks.abort_all();
    while tasks.join_next().await.is_some() {}
    Ok(())
}
