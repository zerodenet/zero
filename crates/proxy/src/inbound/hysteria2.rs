// Hysteria2 inbound — hysteria2.rs

use std::io;
use std::sync::Arc;

use tokio::select;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info, warn};
use zero_config::InboundConfig;
use zero_core::{Network, ProtocolType, Session};
use zero_engine::EngineError;
use zero_protocol_hysteria2::{
    build_auth_error, build_auth_ok, build_connect_error, build_connect_ok,
    parse_auth_frame, parse_tcp_connect_header, Hysteria2Stream,
};
use zero_traits::AsyncSocket;

use crate::runtime::Proxy;

impl Proxy {
    pub(crate) async fn run_hysteria2_listener(
        &self,
        inbound: InboundConfig,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), EngineError> {
        let password = match &inbound.protocol {
            zero_config::InboundProtocolConfig::Hysteria2 { password, .. } => password.clone(),
            _ => {
                return Err(EngineError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "hysteria2 listener requires hysteria2 protocol config",
                )))
            }
        };

        let cert_path = match &inbound.protocol {
            zero_config::InboundProtocolConfig::Hysteria2 { cert_path, .. } => {
                cert_path.clone().unwrap_or_else(|| "certs/fullchain.pem".to_string())
            }
            _ => "certs/fullchain.pem".to_string(),
        };
        let key_path = match &inbound.protocol {
            zero_config::InboundProtocolConfig::Hysteria2 { key_path, .. } => {
                key_path.clone().unwrap_or_else(|| "certs/privkey.pem".to_string())
            }
            _ => "certs/privkey.pem".to_string(),
        };

        let listen_addr = format!("{}:{}", inbound.listen.address, inbound.listen.port);
        let quic_inbound = crate::transport::QuicInbound::bind(
            &listen_addr,
            &cert_path,
            &key_path,
            self.config.source_dir(),
        )
        .await?;

        let mut connections: JoinSet<Result<(), EngineError>> = JoinSet::new();

        info!(
            inbound_tag = %inbound.tag,
            protocol = "hysteria2",
            listen = %listen_addr,
            "inbound listener ready"
        );

        loop {
            select! {
                changed = shutdown.changed() => {
                    match changed {
                        Ok(()) if *shutdown.borrow() => break,
                        Ok(()) => {}
                        Err(_) => break,
                    }
                }
                accept_result = quic_inbound.accept_connection() => {
                    match accept_result {
                        Ok(conn) => {
                            let engine = self.clone();
                            let inbound_tag = inbound.tag.clone();
                            let password = password.clone();

                            connections.spawn(async move {
                                if let Err(error) = engine.handle_hysteria2_connection(
                                    conn, inbound_tag.as_str(), &password,
                                ).await {
                                    error!(error = %error, "hysteria2 connection error");
                                }
                                Ok(())
                            });
                        }
                        Err(error) => {
                            error!(error = %error, "hysteria2 accept error");
                            break;
                        }
                    }
                }
                result = connections.join_next(), if !connections.is_empty() => {
                    if let Some(Err(error)) = result {
                        if !error.is_cancelled() {
                            error!(error = %error, "hysteria2 connection task panicked");
                        }
                    }
                }
            }
        }

        connections.abort_all();
        while let Some(result) = connections.join_next().await {
            if let Err(error) = result {
                if !error.is_cancelled() {
                    error!(error = %error, "hysteria2 connection shutdown error");
                }
            }
        }

        info!(
            inbound_tag = %inbound.tag,
            protocol = "hysteria2",
            "inbound listener stopped"
        );

        Ok(())
    }

    /// Handle a single Hysteria2 QUIC connection.
    async fn handle_hysteria2_connection(
        &self,
        conn: quinn::Connection,
        inbound_tag: &str,
        password: &str,
    ) -> Result<(), EngineError> {
        // Derive salt from TLS keying material
        let mut salt = [0u8; 32];
        if conn
            .export_keying_material(&mut salt, b"hysteria2 auth", &[])
            .is_err()
        {
            return Err(EngineError::Io(io::Error::new(
                io::ErrorKind::Other,
                "quic key export failed",
            )));
        }

        // Wait for auth stream from client
        let (send, recv) = match conn.accept_bi().await {
            Ok(stream) => stream,
            Err(e) => {
                return Err(EngineError::Io(io::Error::other(format!("accept auth stream: {e}"))));
            }
        };

        let mut auth_stream = Hysteria2Stream::new(send, recv);

        // Read auth frame
        let mut auth_buf = [0u8; 64];
        let n = AsyncSocket::read(&mut auth_stream, &mut auth_buf)
            .await
            .map_err(|e| EngineError::Io(io::Error::other(format!("read auth: {e}"))))?;
        if n == 0 {
            return Err(EngineError::Io(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "hysteria2: EOF on auth stream",
            )));
        }

        let client_hmac = parse_auth_frame(&auth_buf[..n])?;

        // Validate HMAC-SHA256(password, salt)
        let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, password.as_bytes());
        if ring::hmac::verify(&key, &salt, &client_hmac).is_err() {
            let err_resp = build_auth_error("authentication failed");
            let _ = AsyncSocket::write_all(&mut auth_stream, &err_resp).await;
            return Err(EngineError::Io(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "hysteria2: auth failed",
            )));
        }

        // Auth success
        let ok_resp = build_auth_ok();
        AsyncSocket::write_all(&mut auth_stream, &ok_resp)
            .await
            .map_err(|e| EngineError::Io(io::Error::other(format!("write auth ok: {e}"))))?;

        // Drop auth stream — it's no longer used
        drop(auth_stream);

        info!(inbound_tag, "hysteria2 auth success");

        // Accept and dispatch streams
        let mut stream_tasks = JoinSet::new();
        let conn = Arc::new(conn);

        loop {
            select! {
                bi = conn.accept_bi() => {
                    match bi {
                        Ok((send, recv)) => {
                            let engine = self.clone();
                            let tag = inbound_tag.to_owned();
                            stream_tasks.spawn(async move {
                                engine.handle_hysteria2_tcp_stream(send, recv, &tag).await
                            });
                        }
                        Err(e) => {
                            warn!(error = %e, "hysteria2 accept_bi error");
                            break;
                        }
                    }
                }
                result = stream_tasks.join_next(), if !stream_tasks.is_empty() => {
                    match result {
                        Some(Err(e)) if !e.is_cancelled() => {
                            error!(error = %e, "hysteria2 stream task panicked");
                        }
                        _ => {}
                    }
                }
            }
        }

        stream_tasks.abort_all();
        Ok(())
    }

    /// Handle a single Hysteria2 TCP stream: parse connect header, route, relay.
    async fn handle_hysteria2_tcp_stream(
        &self,
        send: quinn::SendStream,
        recv: quinn::RecvStream,
        inbound_tag: &str,
    ) -> Result<(), EngineError> {
        let mut stream = Hysteria2Stream::new(send, recv);

        // Read connect header
        let mut header_buf = [0u8; 512];
        let n = AsyncSocket::read(&mut stream, &mut header_buf)
            .await
            .map_err(|_| EngineError::Io(io::Error::other("read connect header")))?;
        if n == 0 {
            return Ok(());
        }

        let (target, port) = parse_tcp_connect_header(&header_buf[..n])?;

        let mut session = Session::new(0, target.clone(), port, Network::Tcp, ProtocolType::Hysteria2);
        self.prepare_session(&mut session, inbound_tag);

        let action = self.route_decision(&session.target);
        let Ok(resolved) = self.resolve_outbound(action) else {
            let err = build_connect_error("no route");
            let _ = AsyncSocket::write_all(&mut stream, &err).await;
            return Ok(());
        };

        let upstream = match self.establish_tcp_outbound(&session, resolved).await {
            Ok(outbound) => match outbound {
                crate::transport::EstablishedTcpOutbound::Direct { upstream, .. } => upstream,
                crate::transport::EstablishedTcpOutbound::Vless { upstream, .. } => upstream,
                crate::transport::EstablishedTcpOutbound::Socks5 { upstream, .. } => upstream,
                crate::transport::EstablishedTcpOutbound::Hysteria2 { upstream, .. } => upstream,
                crate::transport::EstablishedTcpOutbound::Block { .. } => {
                    let err = build_connect_error("blocked");
                    let _ = AsyncSocket::write_all(&mut stream, &err).await;
                    return Ok(());
                }
            },
            Err(_e) => {
                warn!("hysteria2 tcp outbound failed");
                let err = build_connect_error("outbound failed");
                let _ = AsyncSocket::write_all(&mut stream, &err).await;
                return Ok(());
            }
        };

        // Send connect OK
        let ok_resp = build_connect_ok();
        AsyncSocket::write_all(&mut stream, &ok_resp)
            .await
            .map_err(|_| EngineError::Io(io::Error::other("write connect ok")))?;

        // Bidirectional relay
        let (mut up_read, mut up_write) = tokio::io::split(upstream);
        let (mut down_read, mut down_write) = tokio::io::split(stream);

        let upload = tokio::spawn(async move {
            tokio::io::copy(&mut down_read, &mut up_write).await
        });
        let download = tokio::spawn(async move {
            tokio::io::copy(&mut up_read, &mut down_write).await
        });

        let _ = tokio::try_join!(upload, download);
        Ok(())
    }
}
