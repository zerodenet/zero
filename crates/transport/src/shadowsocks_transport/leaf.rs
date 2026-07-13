use core::future::Future;
use std::sync::Arc;

use zero_core::{Address, Session};
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_platform_tokio::TokioSocket;
use zero_traits::DatagramCodec;

use super::{
    apply_shadowsocks_tcp_relay_hop, establish_shadowsocks_tcp_connect,
    ShadowsocksManagedDatagramFlowResume, ShadowsocksManagedUdpFlowConfig,
    ShadowsocksManagedUdpFlowPlan, ShadowsocksManagedUdpPacketPathCarrierDescriptor,
    ShadowsocksManagedUdpPacketPathDatagramSourceBuild, ShadowsocksManagedUdpPacketPathPlan,
    ShadowsocksTransportLeaf,
};
use crate::{MeteredStream, StreamTraffic, TcpRelayStream};

impl<'a> ShadowsocksTransportLeaf<'a> {
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

    pub fn from_resolved_leaf(leaf: &ResolvedLeafOutbound<'a>) -> Option<Self> {
        let ResolvedLeafOutbound::Shadowsocks {
            tag,
            server,
            port,
            password,
            cipher,
        } = leaf
        else {
            return None;
        };
        Some(Self::new(tag, server, *port, cipher, password))
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

    pub fn cipher(&self) -> &str {
        self.cipher
    }

    pub fn password(&self) -> &str {
        self.password
    }

    pub fn flow_resume(&self) -> Result<ShadowsocksManagedDatagramFlowResume, zero_core::Error> {
        self.flow_config().flow_resume()
    }

    pub fn packet_path_carrier_descriptor(
        &self,
    ) -> Result<ShadowsocksManagedUdpPacketPathCarrierDescriptor, zero_core::Error> {
        self.flow_config().packet_path_carrier_descriptor()
    }

    pub fn packet_path_carrier_codec(
        &self,
    ) -> Result<Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>, zero_core::Error> {
        self.flow_config().packet_path_carrier_codec()
    }

    pub fn packet_path_datagram_source_build(
        &self,
    ) -> Result<ShadowsocksManagedUdpPacketPathDatagramSourceBuild, zero_core::Error> {
        self.flow_config().packet_path_datagram_source_build()
    }

    pub fn udp_flow_plan(&self) -> Result<ShadowsocksManagedUdpFlowPlan<'a>, zero_core::Error> {
        Ok(ShadowsocksManagedUdpFlowPlan::new(
            self.tag,
            self.server,
            self.port,
            self.flow_resume()?,
        ))
    }

    pub fn udp_packet_path_plan(
        &self,
    ) -> Result<ShadowsocksManagedUdpPacketPathPlan<'a>, zero_core::Error> {
        Ok(ShadowsocksManagedUdpPacketPathPlan::new(
            self.server,
            self.port,
            self.packet_path_carrier_descriptor()?,
            self.packet_path_carrier_codec()?,
            self.packet_path_datagram_source_build()?,
        ))
    }

    pub async fn open_tcp_stream<OpenSocket, OpenSocketFut, E>(
        &self,
        session: &Session,
        open_socket: OpenSocket,
    ) -> Result<(TcpRelayStream, StreamTraffic), EngineError>
    where
        OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
        OpenSocketFut: Future<Output = Result<TokioSocket, E>> + Send,
        E: Into<EngineError>,
    {
        let upstream = open_socket(self.server, self.port)
            .await
            .map_err(Into::into)?;
        let metered = MeteredStream::new(TcpRelayStream::from(upstream));
        establish_shadowsocks_tcp_connect(metered, session, self.cipher, self.password).await
    }

    pub async fn open_tcp_relay_hop(
        &self,
        stream: TcpRelayStream,
        session: &Session,
    ) -> Result<TcpRelayStream, EngineError> {
        apply_shadowsocks_tcp_relay_hop(stream, session, self.cipher, self.password).await
    }

    fn flow_config(&self) -> ShadowsocksManagedUdpFlowConfig<'a> {
        ShadowsocksManagedUdpFlowConfig::new(
            self.tag,
            self.server,
            self.port,
            self.cipher,
            self.password,
        )
    }
}
