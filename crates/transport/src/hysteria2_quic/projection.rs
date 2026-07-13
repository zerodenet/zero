use crate::outbound_leaf::{ProtocolSessionTcpHandshake, ProtocolTransportLeaf};
use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use super::{
    connect_hysteria2_tcp_outbound, Hysteria2ManagedDatagramFlowResume,
    Hysteria2ManagedUdpFlowConfig, Hysteria2ManagedUdpFlowPlan,
    Hysteria2ManagedUdpPacketPathCarrierBuild, Hysteria2ManagedUdpPacketPathCarrierDescriptor,
    Hysteria2ManagedUdpPacketPathPlan, Hysteria2TransportLeaf,
};

#[cfg(feature = "hysteria2")]
impl<'a> Hysteria2ManagedUdpFlowConfig<'a> {
    pub fn new(
        tag: &'a str,
        server: &'a str,
        port: u16,
        password: &'a str,
        client_fingerprint: Option<&'a str>,
    ) -> Self {
        Self {
            tag,
            server,
            port,
            password,
            client_fingerprint,
        }
    }

    pub fn flow_resume(&self) -> Hysteria2ManagedDatagramFlowResume {
        Hysteria2ManagedDatagramFlowResume::new(
            hysteria2::udp::Hysteria2UdpFlowConfig::new(
                self.tag,
                self.server,
                self.port,
                self.password,
                self.client_fingerprint,
            )
            .flow_resume(),
        )
    }

    pub fn packet_path_carrier_descriptor(&self) -> Hysteria2ManagedUdpPacketPathCarrierDescriptor {
        Hysteria2ManagedUdpPacketPathCarrierDescriptor::new(
            hysteria2::udp::Hysteria2UdpFlowConfig::new(
                self.tag,
                self.server,
                self.port,
                self.password,
                self.client_fingerprint,
            )
            .packet_path_spec()
            .carrier_descriptor(self.server, self.port),
        )
    }

    pub fn packet_path_carrier_build(&self) -> Hysteria2ManagedUdpPacketPathCarrierBuild {
        Hysteria2ManagedUdpPacketPathCarrierBuild::new(
            hysteria2::udp::Hysteria2UdpFlowConfig::new(
                self.tag,
                self.server,
                self.port,
                self.password,
                self.client_fingerprint,
            )
            .packet_path_spec()
            .carrier_build(self.server, self.port),
        )
    }
}

#[cfg(feature = "hysteria2")]
impl<'a> Hysteria2TransportLeaf<'a> {
    pub fn new(
        tag: &'a str,
        server: &'a str,
        port: u16,
        password: &'a str,
        client_fingerprint: Option<&'a str>,
    ) -> Self {
        Self {
            tag,
            server,
            port,
            password,
            client_fingerprint,
        }
    }

    pub fn from_resolved_leaf(leaf: &ResolvedLeafOutbound<'a>) -> Option<Self> {
        let ResolvedLeafOutbound::Hysteria2 {
            tag,
            server,
            port,
            password,
            client_fingerprint,
            ..
        } = leaf
        else {
            return None;
        };
        Some(Self::new(tag, server, *port, password, *client_fingerprint))
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

    pub fn flow_resume(&self) -> Hysteria2ManagedDatagramFlowResume {
        Hysteria2ManagedUdpFlowConfig::new(
            self.tag,
            self.server,
            self.port,
            self.password,
            self.client_fingerprint,
        )
        .flow_resume()
    }

    pub fn packet_path_carrier_descriptor(&self) -> Hysteria2ManagedUdpPacketPathCarrierDescriptor {
        Hysteria2ManagedUdpFlowConfig::new(
            self.tag,
            self.server,
            self.port,
            self.password,
            self.client_fingerprint,
        )
        .packet_path_carrier_descriptor()
    }

    pub fn packet_path_carrier_build(&self) -> Hysteria2ManagedUdpPacketPathCarrierBuild {
        Hysteria2ManagedUdpFlowConfig::new(
            self.tag,
            self.server,
            self.port,
            self.password,
            self.client_fingerprint,
        )
        .packet_path_carrier_build()
    }

    pub fn udp_flow_plan(&self) -> Hysteria2ManagedUdpFlowPlan<'a> {
        Hysteria2ManagedUdpFlowPlan::new(self.tag, self.server, self.port, self.flow_resume())
    }

    pub fn udp_packet_path_plan(&self) -> Hysteria2ManagedUdpPacketPathPlan {
        Hysteria2ManagedUdpPacketPathPlan::new(
            self.packet_path_carrier_descriptor(),
            self.packet_path_carrier_build(),
        )
    }

    pub async fn open_tcp_stream(
        &self,
        session: &Session,
    ) -> Result<crate::TcpRelayStream, EngineError> {
        connect_hysteria2_tcp_outbound(
            session,
            self.server,
            self.port,
            self.password,
            self.client_fingerprint,
        )
        .await
    }
}

#[cfg(feature = "hysteria2")]
impl ProtocolTransportLeaf for Hysteria2TransportLeaf<'_> {
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

#[cfg(feature = "hysteria2")]
#[async_trait::async_trait]
impl ProtocolSessionTcpHandshake for Hysteria2TransportLeaf<'_> {
    fn connect_stage(&self) -> &'static str {
        "connect_upstream_hysteria2"
    }

    async fn connect_session_stream(
        &self,
        session: &Session,
    ) -> Result<crate::TcpRelayStream, EngineError> {
        self.open_tcp_stream(session).await
    }
}

#[cfg(feature = "hysteria2")]
pub fn udp_flow_resume_from_config(
    tag: &str,
    server: &str,
    port: u16,
    password: &str,
    client_fingerprint: Option<&str>,
) -> Hysteria2ManagedDatagramFlowResume {
    Hysteria2ManagedUdpFlowConfig::new(tag, server, port, password, client_fingerprint)
        .flow_resume()
}

#[cfg(feature = "hysteria2")]
pub fn udp_packet_path_carrier_descriptor_from_config(
    tag: &str,
    server: &str,
    port: u16,
    password: &str,
    client_fingerprint: Option<&str>,
) -> Hysteria2ManagedUdpPacketPathCarrierDescriptor {
    Hysteria2ManagedUdpFlowConfig::new(tag, server, port, password, client_fingerprint)
        .packet_path_carrier_descriptor()
}

#[cfg(feature = "hysteria2")]
pub fn udp_packet_path_carrier_build_from_config(
    tag: &str,
    server: &str,
    port: u16,
    password: &str,
    client_fingerprint: Option<&str>,
) -> Hysteria2ManagedUdpPacketPathCarrierBuild {
    Hysteria2ManagedUdpFlowConfig::new(tag, server, port, password, client_fingerprint)
        .packet_path_carrier_build()
}

#[cfg(feature = "hysteria2")]
impl Hysteria2ManagedDatagramFlowResume {
    fn new(protocol: hysteria2::udp::Hysteria2UdpFlowResume) -> Self {
        Self { protocol }
    }

    pub(super) fn connector_flow(
        &self,
        server: &str,
        port: u16,
    ) -> hysteria2::udp::Hysteria2UdpConnectorFlow {
        hysteria2::udp::connector_flow_from_resume(&self.protocol, server, port)
    }

    pub(super) fn into_protocol_resume(self) -> hysteria2::udp::Hysteria2UdpFlowResume {
        self.protocol
    }
}

#[cfg(feature = "hysteria2")]
impl<'a> Hysteria2ManagedUdpFlowPlan<'a> {
    fn new(
        tag: &'a str,
        server: &'a str,
        port: u16,
        resume: Hysteria2ManagedDatagramFlowResume,
    ) -> Self {
        Self {
            tag,
            server,
            port,
            resume,
        }
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

    pub fn into_parts(self) -> (&'a str, &'a str, u16, Hysteria2ManagedDatagramFlowResume) {
        (self.tag, self.server, self.port, self.resume)
    }

    pub fn into_start_plan(
        self,
    ) -> crate::managed_udp::ManagedDatagramStartPlan<'a, Hysteria2ManagedDatagramFlowResume> {
        crate::managed_udp::ManagedDatagramStartPlan::new(
            self.tag,
            self.server,
            self.port,
            self.resume,
        )
    }

    pub fn into_resume(self) -> Hysteria2ManagedDatagramFlowResume {
        self.resume
    }
}

#[cfg(feature = "hysteria2")]
impl Hysteria2ManagedUdpPacketPathPlan {
    fn new(
        carrier_descriptor: Hysteria2ManagedUdpPacketPathCarrierDescriptor,
        carrier_build: Hysteria2ManagedUdpPacketPathCarrierBuild,
    ) -> Self {
        Self {
            carrier_descriptor,
            carrier_build,
        }
    }

    pub fn into_carrier_descriptor(self) -> Hysteria2ManagedUdpPacketPathCarrierDescriptor {
        self.carrier_descriptor
    }

    pub fn into_carrier_build(self) -> Hysteria2ManagedUdpPacketPathCarrierBuild {
        self.carrier_build
    }
}

#[cfg(feature = "hysteria2")]
impl Hysteria2ManagedUdpPacketPathCarrierDescriptor {
    fn new(protocol: hysteria2::udp::Hysteria2UdpPacketPathCarrierDescriptor) -> Self {
        Self { protocol }
    }

    pub fn into_parts(self) -> (String, String, u16) {
        self.protocol.into_parts()
    }
}

#[cfg(feature = "hysteria2")]
impl Hysteria2ManagedUdpPacketPathCarrierBuild {
    fn new(protocol: hysteria2::udp::Hysteria2UdpPacketPathCarrierBuild) -> Self {
        Self { protocol }
    }

    pub(super) fn into_protocol_build(self) -> hysteria2::udp::Hysteria2UdpPacketPathCarrierBuild {
        self.protocol
    }
}
