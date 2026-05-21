use std::time::Instant;

use zero_core::Session;

use super::super::logging::{log_session_accepted, log_session_failed, log_session_finished};
use super::super::runtime::Proxy;
use super::metered::MeteredStream;
use super::stream::{ClientStream, TcpRelayStream};
use super::tcp_outbound::EstablishedTcpOutbound;
use super::tcp_relay::relay_bidirectional_metered_throttled;
use zero_engine::EngineError;
use zero_engine::SessionHandle;
use zero_engine::SessionOutcome;

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub(crate) enum TcpInboundProtocol {
    #[cfg(feature = "inbound-socks5")]
    Socks5,
    #[cfg(feature = "inbound-http-connect")]
    HttpConnect,
    #[cfg(feature = "inbound-vless")]
    Vless,
    #[cfg(feature = "inbound-hysteria2")]
    Hysteria2,
    #[cfg(feature = "inbound-shadowsocks")]
    Shadowsocks,
}

impl Proxy {
    pub(crate) async fn handle_tcp_session<S>(
        &self,
        mut client: MeteredStream<S>,
        inbound_tag: &str,
        mut session: Session,
        inbound_protocol: TcpInboundProtocol,
    ) -> Result<(), EngineError>
    where
        S: ClientStream,
    {
        let source_addr = client.peer_addr().ok();
        self.prepare_session(&mut session, inbound_tag, source_addr);
        let mut session_handle = self.track_session(session.id);
        let started_at = Instant::now();
        self.record_session_inbound_traffic(session.id, client.drain_traffic());

        self.resolve_fake_ip_target(&mut session).await;
        let action = self.route_decision(&session);
        let resolved = match self.resolve_outbound(&action) {
            Ok(resolved) => resolved,
            Err(error) => {
                let record = session_handle.finish(SessionOutcome::Failed);
                log_session_failed(
                    &session,
                    record.as_ref(),
                    "resolve_outbound",
                    started_at.elapsed(),
                    &error,
                    None,
                );
                return Err(error);
            }
        };
        log_session_accepted(&session, &action, self.config.mode.kind());

        match self.establish_tcp_outbound(&session, resolved).await {
            Ok(EstablishedTcpOutbound::Direct { tag, upstream }) => {
                session.outbound_tag = Some(tag);
                self.set_session_outbound(&session);
                self.send_tcp_success_response(inbound_protocol, &mut client)
                    .await?;
                self.relay_tcp_session(TcpRelayContext {
                    client,
                    upstream,
                    session,
                    session_handle,
                    outcome: SessionOutcome::DirectRelayed,
                    started_at,
                    upstream_endpoint: None,
                })
                .await
            }
            Ok(EstablishedTcpOutbound::Block { tag }) => {
                session.outbound_tag = Some(tag);
                self.set_session_outbound(&session);
                self.send_tcp_block_response(inbound_protocol, &mut client)
                    .await;
                self.record_session_inbound_traffic(session.id, client.drain_traffic());
                if let Some(record) = session_handle.finish(SessionOutcome::Blocked) {
                    log_session_finished(&record, None);
                }

                Ok(())
            }
            Ok(EstablishedTcpOutbound::Socks5 {
                tag,
                server,
                port,
                upstream,
            })
            | Ok(EstablishedTcpOutbound::Vless {
                tag,
                server,
                port,
                upstream,
            })
            | Ok(EstablishedTcpOutbound::Hysteria2 {
                tag,
                server,
                port,
                upstream,
            })
            | Ok(EstablishedTcpOutbound::Shadowsocks {
                tag,
                server,
                port,
                upstream,
            })
            | Ok(EstablishedTcpOutbound::Trojan {
                tag,
                server,
                port,
                upstream,
            }) => {
                session.outbound_tag = Some(tag);
                self.set_session_outbound(&session);
                self.send_tcp_success_response(inbound_protocol, &mut client)
                    .await?;
                self.relay_tcp_session(TcpRelayContext {
                    client,
                    upstream,
                    session,
                    session_handle,
                    outcome: SessionOutcome::ChainedRelayed,
                    started_at,
                    upstream_endpoint: Some((server, port)),
                })
                .await
            }
            Ok(EstablishedTcpOutbound::Relay { upstream }) => {
                self.send_tcp_success_response(inbound_protocol, &mut client)
                    .await?;
                self.relay_tcp_session(TcpRelayContext {
                    client,
                    upstream,
                    session,
                    session_handle,
                    outcome: SessionOutcome::ChainedRelayed,
                    started_at,
                    upstream_endpoint: None,
                })
                .await
            }
            Err(failure) => {
                self.send_tcp_upstream_failure_response(inbound_protocol, &mut client)
                    .await;
                self.record_session_inbound_traffic(session.id, client.drain_traffic());
                let record = session_handle.finish(SessionOutcome::Failed);
                log_session_failed(
                    &session,
                    record.as_ref(),
                    failure.stage,
                    started_at.elapsed(),
                    &failure.error,
                    failure
                        .upstream_endpoint
                        .as_ref()
                        .map(|(server, port)| (server.as_str(), *port)),
                );
                Err(failure.error)
            }
        }
    }

    async fn relay_tcp_session<S>(&self, mut context: TcpRelayContext<S>) -> Result<(), EngineError>
    where
        S: ClientStream,
    {
        let session_id = context.session.id;
        let up_bps = context.session.up_bps;
        let down_bps = context.session.down_bps;
        self.record_session_inbound_traffic(session_id, context.client.drain_traffic());
        let client = context.client.into_inner();
        let upload_engine = self.engine().clone();
        let download_engine = self.engine().clone();

        match relay_bidirectional_metered_throttled(
            client,
            context.upstream,
            move |bytes| upload_engine.record_session_upload(session_id, bytes),
            move |bytes| download_engine.record_session_download(session_id, bytes),
            up_bps,
            down_bps,
        )
        .await
        {
            Ok(_) => {
                if let Some(record) = context.session_handle.finish(context.outcome) {
                    log_session_finished(
                        &record,
                        context
                            .upstream_endpoint
                            .as_ref()
                            .map(|(server, port)| (server.as_str(), *port)),
                    );
                }
                Ok(())
            }
            Err(error) => {
                let record = context.session_handle.finish(SessionOutcome::Failed);
                log_session_failed(
                    &context.session,
                    record.as_ref(),
                    "relay",
                    context.started_at.elapsed(),
                    &error,
                    context
                        .upstream_endpoint
                        .as_ref()
                        .map(|(server, port)| (server.as_str(), *port)),
                );
                Err(error.into())
            }
        }
    }

    async fn send_tcp_success_response<S>(
        &self,
        protocol: TcpInboundProtocol,
        client: &mut MeteredStream<S>,
    ) -> Result<(), EngineError>
    where
        S: ClientStream,
    {
        match protocol {
            #[cfg(feature = "inbound-socks5")]
            TcpInboundProtocol::Socks5 => {
                self.protocols
                    .socks5_inbound
                    .send_response(client, zero_protocol_socks5::Socks5Reply::Succeeded)
                    .await?;
            }
            #[cfg(feature = "inbound-http-connect")]
            TcpInboundProtocol::HttpConnect => {
                self.protocols
                    .http_connect_inbound
                    .send_response(
                        client,
                        zero_protocol_http_connect::HttpConnectResponse::ConnectionEstablished,
                    )
                    .await?;
            }
            #[cfg(feature = "inbound-vless")]
            TcpInboundProtocol::Vless => {
                self.protocols.vless_inbound.send_response(client).await?;
            }
            #[cfg(feature = "inbound-hysteria2")]
            TcpInboundProtocol::Hysteria2 => {
                // Hysteria2 uses QUIC, not TCP — this arm is unreachable
            }
            #[cfg(feature = "inbound-shadowsocks")]
            TcpInboundProtocol::Shadowsocks => {}
        }

        Ok(())
    }

    async fn send_tcp_block_response<S>(
        &self,
        protocol: TcpInboundProtocol,
        client: &mut MeteredStream<S>,
    ) where
        S: ClientStream,
    {
        match protocol {
            #[cfg(feature = "inbound-socks5")]
            TcpInboundProtocol::Socks5 => {
                self.reply_and_close_socks5(
                    client,
                    zero_protocol_socks5::Socks5Reply::ConnectionNotAllowed,
                )
                .await;
            }
            #[cfg(feature = "inbound-http-connect")]
            TcpInboundProtocol::HttpConnect => {
                self.reply_and_close_http(
                    client,
                    zero_protocol_http_connect::HttpConnectResponse::Forbidden,
                )
                .await;
            }
            #[cfg(feature = "inbound-vless")]
            TcpInboundProtocol::Vless => {
                self.close_vless_client(client).await;
            }
            #[cfg(feature = "inbound-hysteria2")]
            TcpInboundProtocol::Hysteria2 => {}
            #[cfg(feature = "inbound-shadowsocks")]
            TcpInboundProtocol::Shadowsocks => {}
        }
    }

    async fn send_tcp_upstream_failure_response<S>(
        &self,
        protocol: TcpInboundProtocol,
        client: &mut MeteredStream<S>,
    ) where
        S: ClientStream,
    {
        match protocol {
            #[cfg(feature = "inbound-socks5")]
            TcpInboundProtocol::Socks5 => {
                self.reply_and_close_socks5(
                    client,
                    zero_protocol_socks5::Socks5Reply::HostUnreachable,
                )
                .await;
            }
            #[cfg(feature = "inbound-http-connect")]
            TcpInboundProtocol::HttpConnect => {
                self.reply_and_close_http(
                    client,
                    zero_protocol_http_connect::HttpConnectResponse::BadGateway,
                )
                .await;
            }
            #[cfg(feature = "inbound-vless")]
            TcpInboundProtocol::Vless => {
                self.close_vless_client(client).await;
            }
            #[cfg(feature = "inbound-hysteria2")]
            TcpInboundProtocol::Hysteria2 => {}
            #[cfg(feature = "inbound-shadowsocks")]
            TcpInboundProtocol::Shadowsocks => {}
        }
    }
}

struct TcpRelayContext<S> {
    client: MeteredStream<S>,
    upstream: TcpRelayStream,
    session: Session,
    session_handle: SessionHandle,
    outcome: SessionOutcome,
    started_at: Instant,
    upstream_endpoint: Option<(String, u16)>,
}
