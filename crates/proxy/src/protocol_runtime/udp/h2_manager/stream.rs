use super::super::packet_path_traits::UdpPacketRef;
use super::super::H2UdpPeer;
use super::bridge;
use crate::outbound::hysteria2::Hysteria2Connector;
use std::sync::Arc;
use zero_core::Error;
use zero_engine::EngineError;

pub(super) struct PacketStream {
    pub(super) sender: hysteria2::Hysteria2UdpFlowSender,
    pub(super) recv_tx: bridge::ResponseSender,
}

#[derive(Clone)]
struct Hysteria2QuicDatagramIo {
    conn: Arc<quinn::Connection>,
}

impl hysteria2::Hysteria2UdpDatagramIo for Hysteria2QuicDatagramIo {
    async fn send_datagram(&self, datagram: Vec<u8>) -> Result<(), Error> {
        self.conn
            .send_datagram(datagram.into())
            .map_err(|_| Error::Io("hysteria2: failed to send UDP datagram"))
    }

    async fn read_datagram(&self) -> Result<Vec<u8>, Error> {
        self.conn
            .read_datagram()
            .await
            .map(|data| data.to_vec())
            .map_err(|_| Error::Io("hysteria2: failed to read UDP datagram"))
    }
}

pub(super) async fn establish(
    peer: &H2UdpPeer<'_>,
    initial_packet: UdpPacketRef<'_>,
    resume: hysteria2::Hysteria2UdpFlowResume,
) -> Result<PacketStream, EngineError> {
    let connector_profile = resume.connector_profile();
    let conn = Arc::new(
        Hysteria2Connector::new(
            peer.endpoint.server,
            peer.endpoint.port,
            connector_profile.password(),
        )
        .with_fingerprint(connector_profile.client_fingerprint())
        .connect_raw()
        .await?,
    );
    let initial_packet = hysteria2::udp_flow_packet(
        initial_packet.target,
        initial_packet.port,
        initial_packet.payload,
    );
    let flow = hysteria2::open_udp_flow(Hysteria2QuicDatagramIo { conn }, initial_packet, resume);

    Ok(PacketStream {
        sender: flow.sender,
        recv_tx: flow.responses,
    })
}
