use alloc::boxed::Box;
use alloc::string::String;

use std::future::Future;

use zero_core::Session;
use zero_platform_tokio::TokioSocket;
use zero_transport::outbound_leaf::{ProtocolSocketTcpHandshake, ProtocolTransportLeaf};
use zero_transport::RuntimeError;
use zero_transport::{MeteredStream, StreamTraffic, TcpRelayStream};

use super::{
    apply_socks5_tcp_relay_hop, establish_socks5_tcp_connect, Socks5ManagedUdpFlowConfig,
    Socks5ManagedUdpFlowPlan, Socks5ManagedUdpPacketPathCarrierBuild,
    Socks5ManagedUdpPacketPathCarrierDescriptor, Socks5ManagedUdpPacketPathPlan,
    Socks5TransportLeaf,
};

impl ProtocolTransportLeaf for Socks5TransportLeaf {
    fn tag(&self) -> &str {
        self.tag()
    }
    fn server(&self) -> &str {
        self.server()
    }
    fn port(&self) -> u16 {
        self.port()
    }
}

#[async_trait::async_trait]
impl ProtocolSocketTcpHandshake for Socks5TransportLeaf {
    fn connect_stage(&self) -> &'static str {
        "connect_upstream_socks5"
    }

    async fn handshake_socket(
        &self,
        socket: TokioSocket,
        session: &Session,
    ) -> Result<(TcpRelayStream, StreamTraffic), RuntimeError> {
        let metered = MeteredStream::new(TcpRelayStream::from(socket));
        establish_socks5_tcp_connect(
            metered,
            session,
            self.username.as_deref(),
            self.password.as_deref(),
        )
        .await
    }

    async fn handshake_relay(
        &self,
        stream: TcpRelayStream,
        session: &Session,
    ) -> Result<TcpRelayStream, RuntimeError> {
        apply_socks5_tcp_relay_hop(
            stream,
            session,
            self.username.as_deref(),
            self.password.as_deref(),
        )
        .await
    }
}

impl Socks5TransportLeaf {
    pub fn new(
        tag: impl Into<String>,
        server: impl Into<String>,
        port: u16,
        username: Option<String>,
        password: Option<String>,
    ) -> Self {
        Self {
            tag: tag.into(),
            server: server.into(),
            port,
            username,
            password,
        }
    }

    pub fn tag(&self) -> &str {
        &self.tag
    }

    pub fn server(&self) -> &str {
        &self.server
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn association_target(&self) -> super::Socks5ManagedUdpAssociationTarget {
        self.flow_config().association_target()
    }

    pub fn packet_path_carrier_descriptor(&self) -> Socks5ManagedUdpPacketPathCarrierDescriptor {
        self.flow_config().packet_path_carrier_descriptor()
    }

    pub fn packet_path_carrier_build(&self) -> Socks5ManagedUdpPacketPathCarrierBuild {
        self.flow_config().packet_path_carrier_build()
    }

    pub fn udp_flow_plan(&self) -> Socks5ManagedUdpFlowPlan {
        Socks5ManagedUdpFlowPlan::new(
            self.tag.clone(),
            self.server.clone(),
            self.port,
            self.association_target(),
        )
    }

    pub fn udp_packet_path_plan(&self) -> Socks5ManagedUdpPacketPathPlan {
        Socks5ManagedUdpPacketPathPlan::new(
            self.packet_path_carrier_descriptor(),
            self.packet_path_carrier_build(),
        )
    }

    pub async fn open_tcp_stream<OpenSocket, OpenSocketFut, E>(
        &self,
        session: &Session,
        open_socket: OpenSocket,
    ) -> Result<(TcpRelayStream, StreamTraffic), RuntimeError>
    where
        OpenSocket: FnOnce(&str, u16) -> OpenSocketFut,
        OpenSocketFut: Future<Output = Result<TokioSocket, E>>,
        E: Into<RuntimeError>,
    {
        let upstream = open_socket(&self.server, self.port)
            .await
            .map_err(Into::into)?;
        let metered = MeteredStream::new(TcpRelayStream::from(upstream));
        establish_socks5_tcp_connect(
            metered,
            session,
            self.username.as_deref(),
            self.password.as_deref(),
        )
        .await
    }

    pub async fn open_tcp_relay_hop(
        &self,
        stream: TcpRelayStream,
        session: &Session,
    ) -> Result<TcpRelayStream, RuntimeError> {
        apply_socks5_tcp_relay_hop(
            stream,
            session,
            self.username.as_deref(),
            self.password.as_deref(),
        )
        .await
    }

    fn flow_config(&self) -> Socks5ManagedUdpFlowConfig<'_> {
        Socks5ManagedUdpFlowConfig::new(
            &self.tag,
            &self.server,
            self.port,
            self.username.as_deref(),
            self.password.as_deref(),
        )
    }
}
