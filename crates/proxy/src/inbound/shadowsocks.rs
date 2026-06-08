//! Shadowsocks inbound: listener lifecycle, TCP pipe entry, and UDP pipe entry.

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use shadowsocks::{
    CipherKind, ShadowsocksAeadStream, ShadowsocksDatagramCodec, ShadowsocksInbound,
};
use tokio::net::UdpSocket;
use tokio::select;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info, warn};
use zero_config::InboundConfig;
use zero_core::{Address, Session};
use zero_engine::EngineError;
use zero_traits::DatagramCodec;

use crate::logging::log_listener_connection_error;
use crate::runtime::bind_listener;
use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::pipe::{KernelPipe, UdpPipe, UdpPipeInput};
use crate::runtime::Proxy;
use crate::transport::{MeteredStream, TcpRelayStream};

#[derive(Clone)]
pub(crate) struct ShadowsocksInboundHandler {
    ss_inbound: ShadowsocksInbound,
    cipher: CipherKind,
    password: Vec<u8>,
}

#[async_trait]
impl InboundProtocol for ShadowsocksInboundHandler {
    type ClientStream = ShadowsocksAeadStream<TcpRelayStream>;

    async fn accept(
        &self,
        stream: TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError> {
        let mut metered = MeteredStream::new(stream);
        let accept = self
            .ss_inbound
            .accept_request(&mut metered, self.cipher, &self.password)
            .await?;

        let mut session = accept.session.clone();
        let mut sa = zero_core::SessionAuth::new("shadowsocks");
        sa.principal_key = Some(String::from_utf8_lossy(&self.password).to_string());
        session.apply_auth(sa);

        let client = accept.into_aead_stream(metered.into_inner(), &self.password)?;

        Ok((session, client))
    }

    async fn send_ok(&self, _client: &mut Self::ClientStream) -> Result<(), EngineError> {
        Ok(()) // Shadowsocks has no success response
    }

    async fn send_blocked(&self, _client: &mut Self::ClientStream) -> Result<(), EngineError> {
        Ok(())
    }

    async fn send_upstream_failure(
        &self,
        _client: &mut Self::ClientStream,
    ) -> Result<(), EngineError> {
        Ok(())
    }
}

impl Proxy {
    #[allow(clippy::too_many_lines)]
    pub(crate) async fn run_shadowsocks_listener(
        &self,
        inbound: InboundConfig,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), EngineError> {
        let (password, cipher_str, _up_bps, _down_bps) = match &inbound.protocol {
            zero_config::InboundProtocolConfig::Shadowsocks {
                password,
                cipher,
                up_bps,
                down_bps,
            } => (password.clone(), cipher.clone(), *up_bps, *down_bps),
            _ => {
                return Err(EngineError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "shadowsocks listener requires shadowsocks config",
                )))
            }
        };

        let cipher = CipherKind::from_str(&cipher_str).ok_or_else(|| {
            EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unknown shadowsocks cipher: {cipher_str}"),
            ))
        })?;

        let listener = bind_listener(&inbound).await?;
        let local_addr = listener.local_addr()?;

        let udp_socket = match UdpSocket::bind(&format!(
            "{}:{}",
            inbound.listen.address, inbound.listen.port
        ))
        .await
        {
            Ok(s) => Some(Arc::new(s)),
            Err(e) => {
                warn!(error = %e, "shadowsocks: failed to bind UDP socket, UDP disabled");
                None
            }
        };

        let handler = ShadowsocksInboundHandler {
            ss_inbound: ShadowsocksInbound,
            cipher,
            password: password.clone().into_bytes(),
        };

        let mut connections: JoinSet<Result<(), EngineError>> = JoinSet::new();

        info!(
            inbound_tag = %inbound.tag,
            protocol = "shadowsocks",
            cipher = %cipher_str,
            listen = %local_addr,
            udp = udp_socket.is_some(),
            "inbound listener ready"
        );

        if let Some(udp) = udp_socket.as_ref() {
            let engine = self.clone();
            let tag = inbound.tag.clone();
            let password = password.clone();
            let udp = udp.clone();
            connections
                .spawn(async move { engine.ss_udp_relay_loop(udp, &tag, &password, cipher).await });
        }

        loop {
            select! {
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
                            let engine = self.clone();
                            let tag = inbound.tag.clone();
                            let handler = handler.clone();
                            let source_addr = remote_addr_to_socket(remote_addr);
                            connections.spawn(async move {
                                match handler.accept(stream.into()).await {
                                    Ok((session, client)) => {
                                        let _ = serve_inbound(
                                            &engine, session, client, &handler,
                                            &tag, source_addr,
                                        ).await;
                                    }
                                    Err(error) => {
                                        log_listener_connection_error(
                                            "shadowsocks", &tag, &remote_addr, &error,
                                        );
                                    }
                                }
                                Ok(())
                            });
                        }
                        Err(e) => {
                            error!(error = %e, "shadowsocks: accept error");
                            break;
                        }
                    }
                }
                result = connections.join_next(), if !connections.is_empty() => {
                    match result {
                        Some(Err(error)) if !error.is_cancelled() => {
                            error!(error = %error, "shadowsocks connection task panicked");
                        }
                        _ => {}
                    }
                }
            }
        }

        connections.abort_all();
        while let Some(result) = connections.join_next().await {
            if let Err(error) = result {
                if !error.is_cancelled() {
                    error!(error = %error, "shadowsocks shutdown error");
                }
            }
        }

        info!(inbound_tag = %inbound.tag, protocol = "shadowsocks", "listener stopped");
        Ok(())
    }
}

// UDP relay: protocol framing here, routing through the UDP pipe.

impl Proxy {
    pub(crate) async fn ss_udp_relay_loop(
        &self,
        udp_socket: Arc<UdpSocket>,
        inbound_tag: &str,
        password: &str,
        cipher: CipherKind,
    ) -> Result<(), EngineError> {
        use zero_core::ProtocolType;

        let mut dispatch = crate::runtime::udp_dispatch::UdpDispatch::new(inbound_tag).await?;
        // Map session_id -> client_addr for response delivery.
        let mut client_sessions: std::collections::HashMap<u64, SocketAddr> =
            std::collections::HashMap::new();

        let mut buf = [0u8; 65536];
        let mut direct_buf = [0u8; 65536];

        loop {
            let (direct_sock, chain_tasks) = dispatch.poll_sockets();

            tokio::select! {
                recv = udp_socket.recv_from(&mut buf) => {
                    let (n, client_addr) = match recv {
                        Ok(r) => r,
                        Err(e) => { warn!(error = %e, "ss udp recv error"); break Ok(()); }
                    };
                    let packet = &buf[..n];

                    let codec = ShadowsocksDatagramCodec {
                        cipher,
                        password: password.as_bytes().to_vec(),
                    };
                    let Some((target, port, payload)) =
                        <ShadowsocksDatagramCodec as DatagramCodec<Address>>::decode(
                            &codec, packet,
                        )
                    else {
                        continue;
                    };

                    let mut sa = zero_core::SessionAuth::new("shadowsocks");
                    sa.principal_key = Some(password.to_owned());
                    match UdpPipe::new(self, &mut dispatch)
                        .dispatch(UdpPipeInput {
                            target,
                            port,
                            payload: &payload,
                            protocol: ProtocolType::Shadowsocks,
                            auth: Some(&sa),
                        })
                        .await
                    {
                        Ok(session_id) => {
                            client_sessions.insert(session_id, client_addr);
                        }
                        Err(error) => {
                            warn!(error = %error, "ss udp dispatch failed");
                        }
                    }
                }

                recv = direct_sock.recv_from_addr(&mut direct_buf) => {
                    let (n, sender) = recv?;
                    if let Some(sid) = dispatch.direct_response_session_id(sender) {
                        if let Some(&client) = client_sessions.get(&sid) {
                            ss_send_encrypted(
                                udp_socket.as_ref(), cipher, password,
                                &address_from_socket_addr(sender),
                                sender.port(),
                                &direct_buf[..n],
                                client,
                            ).await;
                        }
                    }
                }

                Some(chain_result) = chain_tasks.join_next() => {
                    match chain_result {
                        Ok(Ok((target, port, payload, session_id))) => {
                            if let Some(sid) = session_id {
                                if let Some(&client) = client_sessions.get(&sid) {
                                    ss_send_encrypted(
                                        udp_socket.as_ref(), cipher, password,
                                        &target, port, &payload, client,
                                    ).await;
                                }
                            }
                        }
                        Ok(Err(error)) => {
                            warn!(error = %error, "ss chain response error");
                        }
                        Err(e) => {
                            warn!(error = %e, "ss chain task panicked");
                        }
                    }
                }
            }
        }
    }
}

/// Encode and send one Shadowsocks UDP response datagram.
async fn ss_send_encrypted(
    socket: &UdpSocket,
    cipher: CipherKind,
    password: &str,
    target: &Address,
    port: u16,
    payload: &[u8],
    client: SocketAddr,
) {
    let codec = ShadowsocksDatagramCodec {
        cipher,
        password: password.as_bytes().to_vec(),
    };
    let Ok(resp) =
        <ShadowsocksDatagramCodec as DatagramCodec<Address>>::encode(&codec, target, port, payload)
    else {
        return;
    };
    let _ = socket.send_to(&resp, client).await;
}
fn remote_addr_to_socket(addr: Option<zero_traits::IpAddress>) -> Option<SocketAddr> {
    addr.map(|ip| match ip {
        zero_traits::IpAddress::V4(octets) => {
            SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::from(octets)), 0)
        }
        zero_traits::IpAddress::V6(octets) => {
            SocketAddr::new(std::net::IpAddr::V6(std::net::Ipv6Addr::from(octets)), 0)
        }
    })
}

fn address_from_socket_addr(addr: SocketAddr) -> Address {
    match addr.ip() {
        std::net::IpAddr::V4(ip) => Address::Ipv4(ip.octets()),
        std::net::IpAddr::V6(ip) => Address::Ipv6(ip.octets()),
    }
}
