use tokio::task::JoinSet;
use zero_core::Address;
use zero_engine::EngineError;

use super::{ChainTask, FlowFailure};

#[cfg(feature = "hysteria2")]
use {
    super::packet_path_traits::{H2UdpPeer, UdpFlowContext, UdpPacketRef, UdpPeerEndpoint},
    crate::transport::Hysteria2Connector,
    hysteria2::{Hysteria2Outbound, Hysteria2UdpPacket, Hysteria2UdpPacketTarget},
    std::collections::HashMap,
    std::sync::Arc,
    tokio::sync::broadcast,
    zero_traits::UdpDatagramFraming,
};

#[cfg(feature = "hysteria2")]
type RecvItem = (Address, u16, Vec<u8>);

#[cfg(feature = "hysteria2")]
pub(crate) struct H2ChainManager {
    upstreams: HashMap<(String, u16, String), H2Entry>,
}

#[cfg(feature = "hysteria2")]
struct H2Entry {
    send_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
}

#[cfg(feature = "hysteria2")]
pub(crate) struct H2SendExisting<'a> {
    pub(crate) chain_tasks: &'a mut JoinSet<ChainTask>,
    pub(crate) session_id: u64,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) password: &'a str,
    pub(crate) client_fingerprint: Option<&'a str>,
    pub(crate) target: &'a Address,
    pub(crate) target_port: u16,
    pub(crate) payload: &'a [u8],
}

#[cfg(feature = "hysteria2")]
impl H2ChainManager {
    pub(super) fn new() -> Self {
        Self {
            upstreams: HashMap::new(),
        }
    }

    async fn send(
        &mut self,
        ctx: UdpFlowContext<'_>,
        peer: H2UdpPeer<'_>,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let sent = packet_ref.payload.len();
        let key = (
            peer.endpoint.server.to_owned(),
            peer.endpoint.port,
            peer.password.to_owned(),
        );

        // Cache hit
        if let Some(entry) = self.upstreams.get(&key) {
            let dg = Self::packet(packet_ref.target, packet_ref.port, packet_ref.payload).map_err(
                |error| FlowFailure {
                    stage: "h2_udp_packet",
                    error,
                    upstream: Some(peer.endpoint.upstream()),
                },
            )?;
            let _ = entry.send_tx.send(dg).await;
            return Ok(sent);
        }

        // Cache miss: establish new upstream.
        let send_tx = Self::establish(ctx.chain_tasks, ctx.session_id, &peer, packet_ref)
            .await
            .map_err(|e| FlowFailure {
                stage: "h2_establish",
                error: e,
                upstream: Some(peer.endpoint.upstream()),
            })?;

        self.upstreams.insert(
            key,
            H2Entry {
                send_tx: send_tx.clone(),
            },
        );

        Ok(sent)
    }

    pub(crate) async fn send_existing(
        &mut self,
        request: H2SendExisting<'_>,
    ) -> Result<usize, FlowFailure> {
        self.send(
            UdpFlowContext {
                chain_tasks: request.chain_tasks,
                session_id: request.session_id,
            },
            H2UdpPeer {
                endpoint: UdpPeerEndpoint {
                    server: request.server,
                    port: request.port,
                },
                password: request.password,
                client_fingerprint: request.client_fingerprint,
            },
            UdpPacketRef {
                target: request.target,
                port: request.target_port,
                payload: request.payload,
            },
        )
        .await
    }

    async fn establish(
        chain_tasks: &mut JoinSet<ChainTask>,
        session_id: u64,
        peer: &H2UdpPeer<'_>,
        initial_packet: UdpPacketRef<'_>,
    ) -> Result<tokio::sync::mpsc::Sender<Vec<u8>>, EngineError> {
        let connector =
            Hysteria2Connector::new(peer.endpoint.server, peer.endpoint.port, peer.password)
                .with_fingerprint(peer.client_fingerprint);
        let conn = Arc::new(connector.connect_raw().await?);

        let (send_tx, mut send_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(32);
        let (recv_tx, _) = broadcast::channel::<RecvItem>(32);

        let target_owned = initial_packet.target.clone();
        let port_owned = initial_packet.port;
        let init_payload = initial_packet.payload.to_vec();

        // Send task: reads outgoing datagrams, sends via QUIC.
        let conn_send = conn.clone();
        tokio::spawn(async move {
            // Send initial payload first.
            if let Ok(dg) = Self::packet(&target_owned, port_owned, &init_payload) {
                if conn_send.send_datagram(dg.into()).is_err() {
                    return;
                }
            }
            while let Some(datagram) = send_rx.recv().await {
                if conn_send.send_datagram(datagram.into()).is_err() {
                    break;
                }
            }
        });

        // Recv task: reads QUIC datagrams, parses target+port, broadcasts.
        let conn_recv = conn.clone();
        let recv_tx2 = recv_tx.clone();
        tokio::spawn(async move {
            while let Ok(data) = conn_recv.read_datagram().await {
                if let Ok(pkt) = Self::decode_packet(&data) {
                    if recv_tx2.send((pkt.target, pkt.port, pkt.payload)).is_err() {
                        break;
                    }
                }
            }
        });

        // Spawn one-shot bridge task for the response.
        let mut recv_rx = recv_tx.subscribe();
        chain_tasks.spawn(async move {
            match recv_rx.recv().await {
                Ok((t, p, payload)) => Ok((t, p, payload, Some(session_id))),
                Err(_) => Err(EngineError::Io(std::io::Error::other("h2 upstream closed"))),
            }
        });

        Ok(send_tx)
    }

    fn packet(target: &Address, target_port: u16, payload: &[u8]) -> Result<Vec<u8>, EngineError> {
        <Hysteria2Outbound as UdpDatagramFraming<Hysteria2UdpPacketTarget<'_>, ()>>::encode_udp_datagram(
            &Hysteria2Outbound,
            &Hysteria2UdpPacketTarget {
                session_id: 0,
                packet_id: 0,
                target,
                port: target_port,
                payload,
            },
        )
        .map_err(EngineError::from)
    }

    fn decode_packet(payload: &[u8]) -> Result<Hysteria2UdpPacket, EngineError> {
        <Hysteria2Outbound as UdpDatagramFraming<Hysteria2UdpPacketTarget<'_>, ()>>::decode_udp_datagram(
            &Hysteria2Outbound,
            &(),
            payload,
        )
        .map_err(EngineError::from)
    }
}

#[cfg(not(feature = "hysteria2"))]
pub(crate) struct H2ChainManager;

#[cfg(not(feature = "hysteria2"))]
impl H2ChainManager {
    pub(super) fn new() -> Self {
        Self
    }

    #[allow(unused_variables)]
    pub(crate) async fn send_existing(
        &mut self,
        _chain_tasks: &mut JoinSet<ChainTask>,
        _session_id: u64,
        _server: &str,
        _port: u16,
        _password: &str,
        _client_fingerprint: Option<&str>,
        _target: &Address,
        _target_port: u16,
        _payload: &[u8],
    ) -> Result<usize, FlowFailure> {
        Err(FlowFailure {
            stage: "h2_feature",
            error: EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Hysteria2 requires feature `hysteria2`",
            )),
            upstream: None,
        })
    }
}
