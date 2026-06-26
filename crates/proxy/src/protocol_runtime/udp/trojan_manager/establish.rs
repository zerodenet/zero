use super::super::TrojanUdpPeer;
use super::connect;
use super::model::{TrojanEntry, TrojanPacket};
use super::stream;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use zero_core::{Address, Session};
use zero_engine::EngineError;

pub(super) async fn direct(
    proxy: &Proxy,
    session: &Session,
    peer: &TrojanUdpPeer<'_>,
    target: &Address,
    target_port: u16,
) -> Result<TrojanEntry, EngineError> {
    let tls_stream = connect::direct_tls_stream(proxy, peer).await?;

    packet_stream(
        proxy,
        session,
        tls_stream,
        peer.password,
        target,
        target_port,
    )
    .await
}

pub(super) async fn over_relay_stream(
    stream: TcpRelayStream,
    tls_server_name: Option<&str>,
    proxy: &Proxy,
    session: &Session,
    peer: &TrojanUdpPeer<'_>,
    target: &Address,
    target_port: u16,
) -> Result<TrojanEntry, EngineError> {
    let tls_stream = connect::relay_tls_stream(stream, tls_server_name, proxy, peer).await?;

    packet_stream(
        proxy,
        session,
        tls_stream,
        peer.password,
        target,
        target_port,
    )
    .await
}

async fn packet_stream(
    proxy: &Proxy,
    session: &Session,
    stream: TcpRelayStream,
    password: &str,
    _target: &Address,
    _target_port: u16,
) -> Result<TrojanEntry, EngineError> {
    let stream::PacketStream { send_tx, recv_tx } =
        stream::spawn_packet_stream(proxy, session, stream, password).await?;

    Ok(TrojanEntry { send_tx, recv_tx })
}

pub(super) fn packet(target: &Address, port: u16, payload: &[u8]) -> TrojanPacket {
    TrojanPacket {
        target: target.clone(),
        port,
        payload: payload.to_vec(),
    }
}
