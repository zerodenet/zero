use super::super::MieruUdpPeer;
use super::connect;
use super::model::MieruEntry;
use super::stream;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use std::sync::Arc;
use zero_core::{Address, Error};
use zero_engine::EngineError;
use zero_traits::DatagramCodec;

pub(super) async fn direct(
    proxy: &Proxy,
    peer: &MieruUdpPeer<'_>,
    codec: Arc<dyn DatagramCodec<Address, Error = Error>>,
) -> Result<MieruEntry, EngineError> {
    let stream = connect::direct_stream(proxy, peer).await?;
    packet_stream(stream, peer.username, peer.password, codec).await
}

pub(super) async fn packet_stream(
    stream: TcpRelayStream,
    username: &str,
    password: &str,
    codec: Arc<dyn DatagramCodec<Address, Error = Error>>,
) -> Result<MieruEntry, EngineError> {
    let connect::EstablishedSession { stream, outbound } =
        connect::establish_udp_associate(stream, username, password).await?;
    let stream::PacketStream { send_tx, recv_tx } =
        stream::spawn_packet_stream(stream, outbound, codec.clone());

    Ok(MieruEntry {
        send_tx,
        recv_tx,
        codec,
    })
}
