#[cfg(feature = "hysteria2")]
use {
    crate::transport::Hysteria2Connector,
    hysteria2::{Hysteria2Outbound, Hysteria2UdpPacket, Hysteria2UdpPacketTarget},
    std::collections::HashMap,
    std::sync::Arc,
    tokio::sync::broadcast,
    zero_traits::UdpDatagramFraming,
};

use tokio::task::JoinSet;
use zero_core::Address;
use zero_engine::EngineError;

use super::{ChainTask, FlowFailure, H2UdpPeer, UdpFlowContext, UdpPacketRef};

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
impl H2ChainManager {
    pub(super) fn new() -> Self {
        Self {
            upstreams: HashMap::new(),
        }
    }

    pub(crate) async fn send(
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
            loop {
                match conn_recv.read_datagram().await {
                    Ok(data) => {
                        if let Ok(pkt) = Self::decode_packet(&data) {
                            if recv_tx2.send((pkt.target, pkt.port, pkt.payload)).is_err() {
                                break;
                            }
                        }
                    }
                    Err(_) => break,
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
    pub(crate) async fn send(
        &mut self,
        _ctx: UdpFlowContext<'_>,
        _peer: H2UdpPeer<'_>,
        _packet_ref: UdpPacketRef<'_>,
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
