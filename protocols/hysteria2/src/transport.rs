use core::future::Future;
use std::io;

use tokio::task::JoinSet;
use zero_core::Session;
use zero_transport::RuntimeError;

mod connection;
mod inbound;
mod managed_udp;
mod model;
mod projection;
mod stream;

pub use connection::open_quic_connection;
pub use inbound::OwnedHysteria2InboundBindPlan;
pub use inbound::{inbound_profile_from_password, inbound_tcp_acceptor};
pub use managed_udp::{
    establish_hysteria2_udp_flow_connection, managed_datagram_connector_flow_from_resume,
    open_hysteria2_udp_packet_path_build,
};
pub use model::{
    Hysteria2AuthenticatedQuicConnection, Hysteria2ManagedDatagramFlowResume,
    Hysteria2ManagedUdpFlowConfig, Hysteria2ManagedUdpFlowPlan,
    Hysteria2ManagedUdpPacketPathCarrierBuild, Hysteria2ManagedUdpPacketPathCarrierDescriptor,
    Hysteria2ManagedUdpPacketPathPlan, Hysteria2TransportLeaf, OwnedHysteria2InboundProfile,
    OwnedHysteria2InboundTcpResponseProtocol,
};
pub use model::{Hysteria2QuicProfile, QuicConnectionOptions};
pub use projection::{
    udp_flow_resume_from_config, udp_packet_path_carrier_build_from_config,
    udp_packet_path_carrier_descriptor_from_config,
};
pub use stream::Hysteria2Stream;

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
        crate::udp::Hysteria2InboundUdpRelay,
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

async fn open_authenticated_hysteria2_quic_connection(
    server: &str,
    port: u16,
    profile: &crate::Hysteria2OutboundProfile,
) -> Result<quinn::Connection, RuntimeError> {
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
        RuntimeError::Io(io::Error::other(format!("hysteria2 open_bi: {error}")))
    })?;
    let mut stream = Hysteria2Stream::new(send, recv);
    profile
        .authenticate_connection(&conn, &mut stream)
        .await
        .map_err(RuntimeError::Core)?;

    Ok(conn)
}

pub async fn connect_hysteria2_tcp_outbound(
    session: &Session,
    server: &str,
    port: u16,
    password: &str,
    client_fingerprint: Option<&str>,
) -> Result<zero_transport::TcpRelayStream, RuntimeError> {
    let profile = crate::outbound_profile_from_config_password(password, client_fingerprint);
    let conn = open_authenticated_hysteria2_quic_connection(server, port, &profile).await?;
    let (send, recv) = conn.open_bi().await.map_err(|error| {
        RuntimeError::Io(io::Error::other(format!("hysteria2 open_bi: {error}")))
    })?;
    let mut stream = Hysteria2Stream::new(send, recv);
    crate::Hysteria2Outbound
        .establish_tcp_connect(&mut stream, session)
        .await
        .map_err(RuntimeError::Core)?;
    Ok(zero_transport::TcpRelayStream::new(stream))
}
