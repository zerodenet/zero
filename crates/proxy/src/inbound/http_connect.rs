use async_trait::async_trait;
use http_connect::{HttpConnectInbound, HttpConnectResponse};
use tokio::io::AsyncWriteExt;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info};
use zero_core::{Error as CoreError, Session};
use zero_engine::EngineError;

use crate::logging::log_listener_connection_error;
use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::Proxy;
use crate::transport::{MeteredStream, TcpRelayStream};

// ── New trait-based handler ────────────────────────────────────────────

#[derive(Clone)]
pub(crate) struct HttpConnectInboundHandler {
    pub(crate) http_connect_inbound: HttpConnectInbound,
}

impl HttpConnectInboundHandler {
    pub(crate) fn http_connect_inbound(&self) -> HttpConnectInbound {
        self.http_connect_inbound
    }
}

#[async_trait]
impl InboundProtocol for HttpConnectInboundHandler {
    type ClientStream = TcpRelayStream;

    async fn accept(
        &self,
        stream: TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError> {
        let mut metered = MeteredStream::new(stream);
        match self.http_connect_inbound.accept_request(&mut metered).await {
            Ok(session) => Ok((session, metered.into_inner())),
            Err(e) => Err(e.into()),
        }
    }

    async fn send_ok(&self, client: &mut TcpRelayStream) -> Result<(), EngineError> {
        self.http_connect_inbound
            .send_response(client, HttpConnectResponse::ConnectionEstablished)
            .await
            .map_err(EngineError::from)
    }

    async fn send_blocked(&self, client: &mut TcpRelayStream) -> Result<(), EngineError> {
        let _ = self
            .http_connect_inbound
            .send_response(client, HttpConnectResponse::Forbidden)
            .await;
        let _ = AsyncWriteExt::shutdown(client).await;
        Ok(())
    }

    async fn send_upstream_failure(&self, client: &mut TcpRelayStream) -> Result<(), EngineError> {
        let _ = self
            .http_connect_inbound
            .send_response(client, HttpConnectResponse::BadGateway)
            .await;
        let _ = AsyncWriteExt::shutdown(client).await;
        Ok(())
    }
    // relay uses default
}

// ── Listener ────────────────────────────────────────────────────────────

pub(crate) async fn run_http_connect_listener_with_bound(
    proxy: &Proxy,
    inbound: zero_config::InboundConfig,
    listener: zero_platform_tokio::TokioListener,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), EngineError> {
    let local_addr = listener.local_addr()?;
    let mut connections = JoinSet::new();

    let handler = HttpConnectInboundHandler {
        http_connect_inbound: http_connect::HttpConnectInbound,
    };

    info!(
        inbound_tag = %inbound.tag,
        protocol = "http_connect",
        listen = %local_addr,
        "inbound listener ready"
    );

    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                match changed {
                    Ok(()) if *shutdown.borrow() => break,
                    Ok(()) => {}
                    Err(_) => break,
                }
            }
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, remote_addr)) => {
                        let engine = proxy.clone();
                        let tag = inbound.tag.clone();
                        let handler = handler.clone();
                        let source_addr = remote_addr_to_socket(remote_addr);
                        connections.spawn(async move {
                            let mut metered = MeteredStream::new(
                                TcpRelayStream::from(stream),
                            );
                            match handler.http_connect_inbound
                                .accept_request(&mut metered).await
                            {
                                Ok(session) => {
                                    // Check for HTTP redirect rewrite rules.
                                    if let Some(resp) = build_redirect_response(
                                        &engine.config.route.url_rewrite,
                                        &session,
                                    ) {
                                        let _ = metered.write_all(resp.as_bytes()).await;
                                    } else {
                                        let _ = serve_inbound(
                                            &engine, session, metered.into_inner(),
                                            &handler, &tag, source_addr,
                                        ).await;
                                    }
                                }
                                Err(CoreError::Unsupported(_)) => {
                                    let _ = handler.http_connect_inbound
                                        .send_response(&mut metered, HttpConnectResponse::MethodNotAllowed)
                                        .await;
                                }
                                Err(CoreError::Protocol(_)) => {
                                    let _ = handler.http_connect_inbound
                                        .send_response(&mut metered, HttpConnectResponse::BadRequest)
                                        .await;
                                }
                                Err(err) => {
                                    let engine_err = EngineError::from(err);
                                    log_listener_connection_error(
                                        "http_connect", &tag, &source_addr, &engine_err,
                                    );
                                }
                            }
                        });
                    }
                    Err(e) => {
                        error!(error = %e, "http_connect: accept error");
                    }
                }
            }
            result = connections.join_next(), if !connections.is_empty() => {
                if let Some(Err(error)) = result {
                    if !error.is_cancelled() {
                        error!(error = %error, "http_connect connection task panicked");
                    }
                }
            }
        }
    }

    connections.abort_all();
    while let Some(result) = connections.join_next().await {
        if let Err(error) = result {
            if !error.is_cancelled() {
                error!(error = %error, "http_connect connection task panicked during shutdown");
            }
        }
    }

    info!(inbound_tag = %inbound.tag, protocol = "http_connect", listen = %local_addr, "inbound listener stopped");
    Ok(())
}

/// Check url_rewrite rules for a redirect (with `status_code` set).
/// Returns the HTTP response string to send, or None if no redirect rule matches.
fn build_redirect_response(
    rules: &[zero_config::UrlRewriteRule],
    session: &Session,
) -> Option<String> {
    let domain = match &session.target {
        zero_core::Address::Domain(d) => d,
        _ => return None,
    };
    for rule in rules {
        let status = rule.status_code?;
        let matched = if let Some(ref from) = rule.from {
            from == domain
        } else if let Some(ref pattern) = rule.from_regex {
            regex::Regex::new(pattern)
                .map(|re| re.is_match(domain))
                .unwrap_or(false)
        } else {
            false
        };
        if matched {
            let location = format!("https://{}:{}", rule.to, session.port);
            return Some(format!(
                "HTTP/1.1 {status} Found\r\nLocation: {location}\r\nConnection: close\r\n\r\n"
            ));
        }
    }
    None
}

fn remote_addr_to_socket(addr: Option<zero_traits::IpAddress>) -> Option<std::net::SocketAddr> {
    addr.map(|ip| match ip {
        zero_traits::IpAddress::V4(octets) => {
            std::net::SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::from(octets)), 0)
        }
        zero_traits::IpAddress::V6(octets) => {
            std::net::SocketAddr::new(std::net::IpAddr::V6(std::net::Ipv6Addr::from(octets)), 0)
        }
    })
}
