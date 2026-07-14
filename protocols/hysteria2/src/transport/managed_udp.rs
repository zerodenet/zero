use std::io;
use std::sync::Arc;

use zero_core::Address;
use zero_traits::DatagramCodec;
use zero_transport::RuntimeError;

use zero_transport::managed_udp::ManagedTupleUdpConnectionOps;

use super::{
    open_quic_connection, Hysteria2ManagedDatagramFlowResume,
    Hysteria2ManagedUdpPacketPathCarrierBuild, Hysteria2QuicProfile, Hysteria2Stream,
    QuicConnectionOptions,
};

impl zero_transport::managed_udp::ProtocolManagedDatagramUdpResumeMetadata
    for Hysteria2ManagedDatagramFlowResume
{
    const ESTABLISH_STAGE: &'static str = "h2_establish";
    const MISMATCH_STAGE: &'static str = "udp_hysteria2_resume";
    const MISMATCH_MESSAGE: &'static str = "expected Hysteria2 UDP flow resume";
}

#[async_trait::async_trait]
impl zero_transport::managed_udp::ProtocolManagedDatagramUdpResumeConnectionOps
    for Hysteria2ManagedDatagramFlowResume
{
    type RawConnection = crate::udp::Hysteria2UdpFlowConnection;

    fn connector_flow_cache_key(&self, server: &str, port: u16) -> String {
        self.connector_flow(server, port).into_cache_key()
    }

    async fn open_protocol_connection(
        &self,
        server: &str,
        port: u16,
        target: &Address,
        target_port: u16,
        payload: &[u8],
    ) -> Result<Self::RawConnection, RuntimeError> {
        let profile = self
            .connector_flow(server, port)
            .into_connection_parts()
            .into_profile();
        let connection = Arc::new(open_udp_profile_connection(server, port, profile).await?);
        Ok(crate::udp::start_udp_flow_with_initial_packet(
            connection,
            target,
            target_port,
            payload,
            self.clone().into_protocol_resume(),
        ))
    }
}

pub fn managed_datagram_connector_flow_from_resume(
    resume: &Hysteria2ManagedDatagramFlowResume,
    server: &str,
    port: u16,
) -> crate::udp::Hysteria2UdpConnectorFlow {
    resume.connector_flow(server, port)
}

async fn open_udp_profile_connection(
    server: &str,
    port: u16,
    profile: crate::udp::Hysteria2UdpConnectorProfile,
) -> Result<quinn::Connection, RuntimeError> {
    let quic_profile = Hysteria2QuicProfile::from_parts(profile.client_fingerprint());
    let connection = open_quic_connection(QuicConnectionOptions {
        server,
        port,
        alpn: vec![b"hysteria2".to_vec()],
        quic_profile,
        datagram_receive_buffer_size: Some(65536),
    })
    .await?;
    let (send, recv) = connection.open_bi().await.map_err(|error| {
        RuntimeError::Io(io::Error::other(format!("hysteria2 open_bi: {error}")))
    })?;
    let mut stream = Hysteria2Stream::new(send, recv);
    profile
        .authenticate_connection(&connection, &mut stream)
        .await
        .map_err(RuntimeError::Core)?;
    Ok(connection)
}

pub async fn open_hysteria2_udp_packet_path_build(
    build: Hysteria2ManagedUdpPacketPathCarrierBuild,
) -> Result<
    (
        quinn::Connection,
        Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
    ),
    RuntimeError,
> {
    let parts = build.into_protocol_build().into_connection_parts();
    let (server, port, profile, codec) = parts.into_shared_codec_parts();
    let connection = open_udp_profile_connection(&server, port, profile).await?;
    Ok((connection, codec))
}

pub async fn establish_hysteria2_udp_flow_connection(
    server: &str,
    port: u16,
    target: &Address,
    target_port: u16,
    payload: &[u8],
    resume: Hysteria2ManagedDatagramFlowResume,
) -> Result<crate::udp::Hysteria2UdpFlowConnection, RuntimeError> {
    let flow = managed_datagram_connector_flow_from_resume(&resume, server, port);
    let profile = flow.into_connection_parts().into_profile();
    let connection = Arc::new(open_udp_profile_connection(server, port, profile).await?);
    Ok(crate::udp::start_udp_flow_with_initial_packet(
        connection,
        target,
        target_port,
        payload,
        resume.into_protocol_resume(),
    ))
}

#[async_trait::async_trait]
impl ManagedTupleUdpConnectionOps for crate::udp::Hysteria2UdpFlowConnection {
    type SendError = zero_core::Error;

    async fn send_protocol_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Self::SendError> {
        crate::udp::Hysteria2UdpFlowConnection::send(self, target, port, payload).await
    }

    fn subscribe_protocol_packets(&self) -> crate::udp::Hysteria2UdpFlowResponseReceiver {
        crate::udp::Hysteria2UdpFlowConnection::subscribe_responses(self)
    }

    fn closed_message_for_connection(&self) -> &'static str {
        "h2 upstream closed"
    }
}
