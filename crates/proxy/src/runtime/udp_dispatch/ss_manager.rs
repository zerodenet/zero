use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use tokio::sync::oneshot;
use tokio::task::JoinSet;
use zero_core::Address;
use zero_engine::EngineError;

use super::{ChainTask, FlowFailure};

type SsRecvItem = (Address, u16, Vec<u8>);

struct SsResponseWaiter {
    target: Address,
    port: u16,
    tx: oneshot::Sender<SsRecvItem>,
}

struct SsUpstream {
    socket: Arc<tokio::net::UdpSocket>,
    waiters: Mutex<VecDeque<SsResponseWaiter>>,
}

pub(super) struct SsChainManager {
    upstreams: HashMap<(String, u16, String, String), Arc<SsUpstream>>,
}

impl SsChainManager {
    pub(super) fn new() -> Self {
        Self {
            upstreams: HashMap::new(),
        }
    }

    pub(super) async fn send(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        session_id: u64,
        server: &str,
        port: u16,
        password: &str,
        cipher: &str,
        target: &Address,
        target_port: u16,
        payload: &[u8],
    ) -> Result<usize, FlowFailure> {
        use shadowsocks::{
            CipherKind, ShadowsocksOutbound, ShadowsocksUdpDecodeContext,
            ShadowsocksUdpPacketTarget,
        };
        use zero_traits::UdpDatagramFraming;

        let cipher_kind = CipherKind::from_str(cipher).ok_or_else(|| FlowFailure {
            stage: "ss_cipher",
            error: EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("unknown shadowsocks cipher: {cipher}"),
            )),
            upstream: Some((server.to_owned(), port)),
        })?;

        let entry = self.ensure_entry(server, port, password, cipher_kind);

        let packet = <ShadowsocksOutbound as UdpDatagramFraming<
            ShadowsocksUdpPacketTarget,
            ShadowsocksUdpDecodeContext,
        >>::encode_udp_datagram(
            &ShadowsocksOutbound,
            &ShadowsocksUdpPacketTarget {
                target,
                port: target_port,
                payload,
                cipher: cipher_kind,
                password: password.as_bytes(),
            },
        )
        .map_err(|e| FlowFailure {
            stage: "ss_encode",
            error: EngineError::Io(std::io::Error::other(e)),
            upstream: Some((server.to_owned(), port)),
        })?;

        let target_addr: SocketAddr =
            format!("{server}:{port}")
                .parse()
                .map_err(|_| FlowFailure {
                    stage: "ss_parse_addr",
                    error: EngineError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("invalid ss upstream: {server}:{port}"),
                    )),
                    upstream: Some((server.to_owned(), port)),
                })?;

        let (response_tx, response_rx) = oneshot::channel();
        entry
            .waiters
            .lock()
            .expect("ss waiters lock poisoned")
            .push_back(SsResponseWaiter {
                target: target.clone(),
                port: target_port,
                tx: response_tx,
            });
        if let Err(e) = entry.socket.send_to(&packet, target_addr).await {
            remove_waiter(&entry.waiters, target, target_port);
            return Err(FlowFailure {
                stage: "ss_send",
                error: EngineError::from(e),
                upstream: Some((server.to_owned(), port)),
            });
        }

        // Spawn one-shot bridge task.
        chain_tasks.spawn(async move {
            match response_rx.await {
                Ok((resp_target, resp_port, resp_payload)) => {
                    Ok((resp_target, resp_port, resp_payload, Some(session_id)))
                }
                Err(_) => Err(EngineError::Io(std::io::Error::other("ss upstream closed"))),
            }
        });

        Ok(payload.len())
    }

    fn ensure_entry(
        &mut self,
        server: &str,
        port: u16,
        password: &str,
        cipher_kind: shadowsocks::CipherKind,
    ) -> Arc<SsUpstream> {
        let key = (
            server.to_owned(),
            port,
            format!("{cipher_kind:?}"),
            password.to_owned(),
        );
        if let Some(entry) = self.upstreams.get(&key) {
            return entry.clone();
        }

        let socket = Arc::new({
            let socket = std::net::UdpSocket::bind("0.0.0.0:0").expect("ss: bind");
            socket.set_nonblocking(true).expect("ss: nonblocking");
            tokio::net::UdpSocket::from_std(socket).expect("ss: tokio")
        });

        let entry = Arc::new(SsUpstream {
            socket: socket.clone(),
            waiters: Mutex::new(VecDeque::new()),
        });
        self.upstreams.insert(key, entry.clone());

        tokio::spawn(Self::recv_loop(
            socket,
            cipher_kind,
            password.to_owned(),
            entry.clone(),
        ));
        entry
    }

    async fn recv_loop(
        socket: Arc<tokio::net::UdpSocket>,
        cipher: shadowsocks::CipherKind,
        password: String,
        upstream: Arc<SsUpstream>,
    ) {
        use shadowsocks::{
            ShadowsocksOutbound, ShadowsocksUdpDecodeContext, ShadowsocksUdpPacketTarget,
        };
        use zero_traits::UdpDatagramFraming;
        let mut buf = vec![0u8; 4096];
        loop {
            let (n, _) = match socket.recv_from(&mut buf).await {
                Ok(r) => r,
                Err(_) => break,
            };
            let packet = &buf[..n];
            let Ok(decoded) = <ShadowsocksOutbound as UdpDatagramFraming<
                ShadowsocksUdpPacketTarget,
                ShadowsocksUdpDecodeContext,
            >>::decode_udp_datagram(
                &ShadowsocksOutbound,
                &ShadowsocksUdpDecodeContext {
                    cipher,
                    password: password.as_bytes(),
                },
                packet,
            ) else {
                continue;
            };
            let waiter = remove_waiter(&upstream.waiters, &decoded.target, decoded.port);
            if let Some(waiter) = waiter {
                let _ = waiter
                    .tx
                    .send((decoded.target, decoded.port, decoded.payload));
            }
        }
    }
}

fn remove_waiter(
    waiters: &Mutex<VecDeque<SsResponseWaiter>>,
    target: &Address,
    port: u16,
) -> Option<SsResponseWaiter> {
    let mut waiters = waiters.lock().expect("ss waiters lock poisoned");
    let index = waiters
        .iter()
        .position(|waiter| waiter.target == *target && waiter.port == port)?;
    waiters.remove(index)
}
