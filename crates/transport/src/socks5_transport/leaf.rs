use std::future::Future;

use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_platform_tokio::TokioSocket;

use super::{
    apply_socks5_tcp_relay_hop, establish_socks5_tcp_connect, Socks5ManagedUdpFlowConfig,
    Socks5ManagedUdpFlowPlan, Socks5ManagedUdpPacketPathCarrierBuild,
    Socks5ManagedUdpPacketPathCarrierDescriptor, Socks5ManagedUdpPacketPathPlan,
    Socks5TransportLeaf,
};
use crate::{MeteredStream, StreamTraffic, TcpRelayStream};

impl<'a> Socks5TransportLeaf<'a> {
    pub fn new(
        tag: &'a str,
        server: &'a str,
        port: u16,
        username: Option<&'a str>,
        password: Option<&'a str>,
    ) -> Self {
        Self {
            tag,
            server,
            port,
            username,
            password,
        }
    }

    pub fn from_resolved_leaf(leaf: &ResolvedLeafOutbound<'a>) -> Option<Self> {
        let ResolvedLeafOutbound::Socks5 {
            tag,
            server,
            port,
            username,
            password,
        } = leaf
        else {
            return None;
        };
        Some(Self::new(tag, server, *port, *username, *password))
    }

    pub fn tag(&self) -> &str {
        self.tag
    }

    pub fn server(&self) -> &str {
        self.server
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

    pub fn udp_flow_plan(&self) -> Socks5ManagedUdpFlowPlan<'a> {
        Socks5ManagedUdpFlowPlan::new(self.tag, self.server, self.port, self.association_target())
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
    ) -> Result<(TcpRelayStream, StreamTraffic), EngineError>
    where
        OpenSocket: FnOnce(&str, u16) -> OpenSocketFut,
        OpenSocketFut: Future<Output = Result<TokioSocket, E>>,
        E: Into<EngineError>,
    {
        let upstream = open_socket(self.server, self.port)
            .await
            .map_err(Into::into)?;
        let metered = MeteredStream::new(TcpRelayStream::from(upstream));
        establish_socks5_tcp_connect(metered, session, self.username, self.password).await
    }

    pub async fn open_tcp_relay_hop(
        &self,
        stream: TcpRelayStream,
        session: &Session,
    ) -> Result<TcpRelayStream, EngineError> {
        apply_socks5_tcp_relay_hop(stream, session, self.username, self.password).await
    }

    fn flow_config(&self) -> Socks5ManagedUdpFlowConfig<'a> {
        Socks5ManagedUdpFlowConfig::new(
            self.tag,
            self.server,
            self.port,
            self.username,
            self.password,
        )
    }
}
