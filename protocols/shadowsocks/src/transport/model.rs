use std::sync::Arc;

use zero_core::Address;
use zero_traits::DatagramCodec;

pub type ShadowsocksUdpResponse = (Address, u16, Vec<u8>);

#[derive(Debug, Clone)]
pub struct ShadowsocksManagedDatagramFlowResume {
    pub(super) protocol: crate::udp::ShadowsocksUdpFlowResume,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowsocksManagedUdpPacketPathCarrierDescriptor {
    pub(super) protocol: crate::udp::ShadowsocksUdpPacketPathCarrierDescriptor,
}

#[derive(Debug, Clone)]
pub struct ShadowsocksManagedUdpPacketPathDatagramSourceBuild {
    pub(super) protocol: crate::udp::ShadowsocksUdpPacketPathDatagramSourceBuild,
}

#[derive(Debug, Clone)]
pub struct ShadowsocksManagedUdpFlowPlan<'a> {
    pub(super) tag: &'a str,
    pub(super) server: &'a str,
    pub(super) port: u16,
    pub(super) resume: ShadowsocksManagedDatagramFlowResume,
}

#[derive(Clone)]
pub struct ShadowsocksManagedUdpPacketPathPlan<'a> {
    pub(super) server: &'a str,
    pub(super) port: u16,
    pub(super) carrier_descriptor: ShadowsocksManagedUdpPacketPathCarrierDescriptor,
    pub(super) carrier_codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
    pub(super) datagram_source: ShadowsocksManagedUdpPacketPathDatagramSourceBuild,
}

#[derive(Debug, Clone, Copy)]
pub struct ShadowsocksManagedUdpFlowConfig<'a> {
    pub(super) tag: &'a str,
    pub(super) server: &'a str,
    pub(super) port: u16,
    pub(super) cipher: &'a str,
    pub(super) password: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct ShadowsocksTransportLeaf<'a> {
    pub(super) tag: &'a str,
    pub(super) server: &'a str,
    pub(super) port: u16,
    pub(super) cipher: &'a str,
    pub(super) password: &'a str,
}

#[derive(Debug, Clone)]
pub struct OwnedShadowsocksInboundProfile {
    pub(super) protocol: crate::ShadowsocksInboundProfile,
}

#[derive(Clone)]
pub struct OwnedShadowsocksInboundTcpAcceptor {
    pub(super) protocol: crate::ShadowsocksInboundTcpAcceptor,
}

pub struct OwnedShadowsocksInboundBindings {
    pub(super) acceptor: OwnedShadowsocksInboundTcpAcceptor,
    pub(super) udp_relay: crate::udp::ShadowsocksInboundUdpRelay,
}

impl<'a> ShadowsocksManagedUdpFlowConfig<'a> {
    pub fn new(
        tag: &'a str,
        server: &'a str,
        port: u16,
        cipher: &'a str,
        password: &'a str,
    ) -> Self {
        Self {
            tag,
            server,
            port,
            cipher,
            password,
        }
    }

    pub fn flow_resume(&self) -> Result<ShadowsocksManagedDatagramFlowResume, zero_core::Error> {
        self.protocol_config()
            .flow_resume()
            .map(ShadowsocksManagedDatagramFlowResume::new)
    }

    pub fn packet_path_carrier_descriptor(
        &self,
    ) -> Result<ShadowsocksManagedUdpPacketPathCarrierDescriptor, zero_core::Error> {
        Ok(ShadowsocksManagedUdpPacketPathCarrierDescriptor::new(
            self.protocol_config()
                .packet_path_spec()?
                .carrier_descriptor(self.server, self.port),
        ))
    }

    pub fn packet_path_carrier_codec(
        &self,
    ) -> Result<Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>, zero_core::Error> {
        Ok(self.protocol_config().packet_path_spec()?.carrier_codec())
    }

    pub fn packet_path_datagram_source_build(
        &self,
    ) -> Result<ShadowsocksManagedUdpPacketPathDatagramSourceBuild, zero_core::Error> {
        Ok(ShadowsocksManagedUdpPacketPathDatagramSourceBuild::new(
            self.protocol_config()
                .packet_path_spec()?
                .datagram_source_build(self.tag, self.server, self.port),
        ))
    }

    fn protocol_config(&self) -> crate::udp::ShadowsocksUdpFlowConfig<'a> {
        crate::udp::ShadowsocksUdpFlowConfig::new(
            self.tag,
            self.server,
            self.port,
            self.cipher,
            self.password,
        )
    }
}

impl ShadowsocksManagedDatagramFlowResume {
    fn new(protocol: crate::udp::ShadowsocksUdpFlowResume) -> Self {
        Self { protocol }
    }

    pub(super) fn socket_flow_spec(&self) -> crate::udp::ShadowsocksUdpSocketFlowSpec {
        crate::udp::managed_socket_flow_from_resume(&self.protocol)
    }

    pub(super) fn into_shared_managed_socket_flow_codec(
        self,
    ) -> Arc<dyn DatagramCodec<Address, Error = zero_core::Error>> {
        self.protocol.into_shared_managed_socket_flow_codec()
    }
}

impl<'a> ShadowsocksManagedUdpFlowPlan<'a> {
    pub(super) fn new(
        tag: &'a str,
        server: &'a str,
        port: u16,
        resume: ShadowsocksManagedDatagramFlowResume,
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

    pub fn into_parts(self) -> (&'a str, &'a str, u16, ShadowsocksManagedDatagramFlowResume) {
        (self.tag, self.server, self.port, self.resume)
    }

    pub fn into_start_plan(
        self,
    ) -> zero_transport::managed_udp::ManagedDatagramStartPlan<
        'a,
        ShadowsocksManagedDatagramFlowResume,
    > {
        zero_transport::managed_udp::ManagedDatagramStartPlan::new(
            self.tag,
            self.server,
            self.port,
            self.resume,
        )
    }

    pub fn into_resume(self) -> ShadowsocksManagedDatagramFlowResume {
        self.resume
    }
}

impl<'a> ShadowsocksManagedUdpPacketPathPlan<'a> {
    pub(super) fn new(
        server: &'a str,
        port: u16,
        carrier_descriptor: ShadowsocksManagedUdpPacketPathCarrierDescriptor,
        carrier_codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
        datagram_source: ShadowsocksManagedUdpPacketPathDatagramSourceBuild,
    ) -> Self {
        Self {
            server,
            port,
            carrier_descriptor,
            carrier_codec,
            datagram_source,
        }
    }

    pub fn server(&self) -> &str {
        self.server
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn carrier_codec(&self) -> Arc<dyn DatagramCodec<Address, Error = zero_core::Error>> {
        self.carrier_codec.clone()
    }

    pub fn into_carrier_descriptor(self) -> ShadowsocksManagedUdpPacketPathCarrierDescriptor {
        self.carrier_descriptor
    }

    pub fn into_datagram_source_build(self) -> ShadowsocksManagedUdpPacketPathDatagramSourceBuild {
        self.datagram_source
    }
}

impl ShadowsocksManagedUdpPacketPathCarrierDescriptor {
    fn new(protocol: crate::udp::ShadowsocksUdpPacketPathCarrierDescriptor) -> Self {
        Self { protocol }
    }

    pub fn into_parts(self) -> (String, String, u16) {
        self.protocol.into_parts()
    }
}

impl ShadowsocksManagedUdpPacketPathDatagramSourceBuild {
    fn new(protocol: crate::udp::ShadowsocksUdpPacketPathDatagramSourceBuild) -> Self {
        Self { protocol }
    }

    pub fn into_shared_codec_parts(
        self,
    ) -> (
        String,
        String,
        u16,
        String,
        Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
    ) {
        self.protocol.into_shared_codec_parts()
    }
}
