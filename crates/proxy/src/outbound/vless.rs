//! VLESS outbound protocol implementation

use std::collections::HashMap;

use tokio::sync::mpsc;
use tokio::task::JoinSet;
use zero_core::{Address, Session};
use zero_engine::EngineError;
use zero_protocol_vless::parse_uuid;
use zero_traits::AsyncSocket;

use crate::runtime::Proxy;
use crate::transport::MeteredStream;

/// VLESS UDP upstream connection handle
#[derive(Clone)]
pub struct VlessUdpUpstream {
    pub session_id: u64,
    pub send_tx: mpsc::Sender<Vec<u8>>,
}

/// Establishes a VLESS UDP upstream connection
pub async fn establish_vless_udp_upstream(
    proxy: &Proxy,
    session: &Session,
    server: &str,
    port: u16,
    id: &str,
    initial_payload: &[u8],
) -> Result<(VlessUdpUpstream, mpsc::Receiver<Vec<u8>>), EngineError> {
    let socket = proxy
        .protocols
        .direct_outbound
        .connect_host(server, port, &proxy.resolver)
        .await?;

    let mut stream = MeteredStream::new(socket);
    let vless_id = parse_uuid(id)?;

    proxy
        .protocols
        .vless_outbound
        .send_udp_request(&mut stream, session, &vless_id)
        .await?;

    stream.write_all(initial_payload).await?;

    let (send_tx, mut send_rx) = mpsc::channel::<Vec<u8>>(32);
    let (recv_tx, recv_rx) = mpsc::channel::<Vec<u8>>(32);

    proxy.record_session_outbound_tx(session.id, initial_payload.len() as u64);

    // Spawn bidirectional forwarder
    let proxy_clone = proxy.clone();
    let session_id = session.id;
    tokio::spawn(async move {
        let mut buffer = vec![0_u8; 64 * 1024];
        loop {
            tokio::select! {
                to_send = send_rx.recv() => {
                    match to_send {
                        Some(payload) => {
                            if stream.write_all(&payload).await.is_err() {
                                break;
                            }
                            proxy_clone.record_session_outbound_tx(session_id, payload.len() as u64);
                        }
                        None => break,
                    }
                }
                read = stream.read(&mut buffer) => {
                    match read {
                        Ok(0) => break,
                        Ok(n) => {
                            if recv_tx.send(buffer[..n].to_vec()).await.is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        }
    });

    Ok((VlessUdpUpstream {
        session_id: session.id,
        send_tx,
    }, recv_rx))
}

/// VLESS UDP outbound manager (for future unified architecture)
#[allow(dead_code)]
pub struct VlessUdpOutboundManager {
    upstreams: HashMap<(Address, u16), VlessUdpUpstream>,
    response_tasks: JoinSet<Result<(Address, u16, Vec<u8>, Option<u64>), EngineError>>,
}

#[allow(dead_code)]
impl VlessUdpOutboundManager {
    /// Get or create an upstream for a target
    pub async fn get_or_create_upstream(
        &mut self,
        proxy: &Proxy,
        session: Session,
        target: Address,
        port: u16,
        server: String,
        server_port: u16,
        id: String,
        initial_payload: Vec<u8>,
    ) -> Result<(), EngineError> {
        let key = (target.clone(), port);

        if let Some(upstream) = self.upstreams.get(&key) {
            proxy.record_session_inbound_rx(upstream.session_id, initial_payload.len() as u64);
            let payload_len = initial_payload.len() as u64;
            let _ = upstream.send_tx.send(initial_payload).await;
            proxy.record_session_outbound_tx(upstream.session_id, payload_len);
            return Ok(());
        }

        match establish_vless_udp_upstream(
            proxy,
            &session,
            &server,
            server_port,
            &id,
            &initial_payload,
        ).await {
            Ok((upstream, mut recv_rx)) => {
                let session_id = upstream.session_id;
                self.upstreams.insert(key, upstream);

                // Spawn response reader task
                self.response_tasks.spawn(async move {
                    loop {
                        let payload = recv_rx.recv().await
                            .ok_or_else(|| EngineError::Io(std::io::Error::other("upstream channel closed")))?;

                        // Use the same target/port as the original request
                        return Ok((target, port, payload, Some(session_id)));
                    }
                });

                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    /// Poll for next response
    pub async fn next_response(&mut self) -> Option<Result<(Address, u16, Vec<u8>, Option<u64>), EngineError>> {
        self.response_tasks.join_next().await.map(|res| match res {
            Ok(inner) => inner,
            Err(e) => Err(EngineError::Io(std::io::Error::other(format!("upstream task failed: {}", e)))),
        })
    }
}
