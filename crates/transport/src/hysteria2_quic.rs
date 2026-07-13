#[cfg(feature = "hysteria2")]
use core::future::Future;
#[cfg(feature = "hysteria2")]
use std::io;

#[cfg(feature = "hysteria2")]
use tokio::task::JoinSet;
#[cfg(feature = "hysteria2")]
use zero_core::Session;
#[cfg(feature = "hysteria2")]
use zero_engine::EngineError;

mod connection;
mod inbound;
#[cfg(feature = "hysteria2")]
mod managed_udp;
mod model;
#[cfg(feature = "hysteria2")]
mod projection;
mod stream;

pub use connection::open_quic_connection;
pub use inbound::OwnedHysteria2InboundBindPlan;
#[cfg(feature = "hysteria2")]
pub use inbound::{inbound_profile_from_protocol, inbound_tcp_acceptor};
#[cfg(feature = "hysteria2")]
pub use managed_udp::{
    establish_hysteria2_udp_flow_connection, managed_datagram_connector_flow_from_resume,
    open_hysteria2_udp_packet_path_build,
};
#[cfg(feature = "hysteria2")]
pub use model::{
    Hysteria2ManagedDatagramFlowResume, Hysteria2ManagedUdpFlowConfig, Hysteria2ManagedUdpFlowPlan,
    Hysteria2ManagedUdpPacketPathCarrierBuild, Hysteria2ManagedUdpPacketPathCarrierDescriptor,
    Hysteria2ManagedUdpPacketPathPlan, Hysteria2TransportLeaf, OwnedHysteria2InboundProfile,
    OwnedHysteria2InboundTcpResponseProtocol,
};
pub use model::{Hysteria2QuicProfile, QuicConnectionOptions};
#[cfg(feature = "hysteria2")]
pub use projection::{
    udp_flow_resume_from_config, udp_packet_path_carrier_build_from_config,
    udp_packet_path_carrier_descriptor_from_config,
};
pub use stream::Hysteria2Stream;

#[cfg(feature = "hysteria2")]
pub async fn accept_and_dispatch_authenticated_hysteria2_quic_session<
    Udp,
    UdpFut,
    Tcp,
    TcpFut,
    TaskResult,
    TaskResultFut,
    E,
>(
    profile: &OwnedHysteria2InboundProfile,
    conn: quinn::Connection,
    on_udp_session: Udp,
    on_tcp_stream: Tcp,
    on_stream_task_result: TaskResult,
) -> Result<(), E>
where
    Udp: FnMut(
        std::sync::Arc<quinn::Connection>,
        hysteria2::udp::Hysteria2InboundUdpRelay,
        &mut JoinSet<Result<(), E>>,
    ) -> UdpFut,
    UdpFut: Future<Output = Result<(), E>>,
    Tcp: FnMut(Session, Hysteria2Stream, &mut JoinSet<Result<(), E>>) -> TcpFut,
    TcpFut: Future<Output = Result<(), E>>,
    TaskResult: FnMut(Result<Result<(), E>, tokio::task::JoinError>) -> TaskResultFut,
    TaskResultFut: Future<Output = Result<(), E>>,
    E: From<zero_core::Error> + Send + 'static,
{
    profile
        .protocol
        .accept_and_dispatch_authenticated_quic_session(
            conn,
            Hysteria2Stream::new,
            on_udp_session,
            on_tcp_stream,
            on_stream_task_result,
        )
        .await
}

#[cfg(feature = "hysteria2")]
async fn open_authenticated_hysteria2_quic_connection(
    server: &str,
    port: u16,
    profile: &hysteria2::Hysteria2OutboundProfile,
) -> Result<quinn::Connection, EngineError> {
    let quic_profile = Hysteria2QuicProfile::from_parts(profile.client_fingerprint());
    let conn = open_quic_connection(QuicConnectionOptions {
        server,
        port,
        alpn: vec![b"hysteria2".to_vec()],
        quic_profile,
        datagram_receive_buffer_size: Some(65536),
    })
    .await?;

    let (send, recv) = conn.open_bi().await.map_err(|error| {
        EngineError::Io(io::Error::other(format!("hysteria2 open_bi: {error}")))
    })?;
    let mut stream = Hysteria2Stream::new(send, recv);
    profile
        .authenticate_connection(&conn, &mut stream)
        .await
        .map_err(EngineError::Core)?;

    Ok(conn)
}

#[cfg(feature = "hysteria2")]
pub async fn connect_hysteria2_tcp_outbound(
    session: &Session,
    server: &str,
    port: u16,
    password: &str,
    client_fingerprint: Option<&str>,
) -> Result<crate::TcpRelayStream, EngineError> {
    let profile = hysteria2::outbound_profile_from_config_password(password, client_fingerprint);
    let conn = open_authenticated_hysteria2_quic_connection(server, port, &profile).await?;
    let (send, recv) = conn.open_bi().await.map_err(|error| {
        EngineError::Io(io::Error::other(format!("hysteria2 open_bi: {error}")))
    })?;
    let mut stream = Hysteria2Stream::new(send, recv);
    hysteria2::Hysteria2Outbound
        .establish_tcp_connect(&mut stream, session)
        .await
        .map_err(EngineError::Core)?;
    Ok(crate::TcpRelayStream::new(stream))
}
