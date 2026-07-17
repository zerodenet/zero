use core::future::Future;
use std::sync::Arc;

use zero_core::{Address, Session};
use zero_platform_tokio::TokioSocket;
use zero_traits::DatagramCodec;
use zero_transport::RuntimeError;
use zero_transport::{MeteredStream, StreamTraffic, TcpRelayStream};

use super::{
    apply_shadowsocks_tcp_relay_hop, establish_shadowsocks_tcp_connect,
    ShadowsocksManagedDatagramFlowResume, ShadowsocksManagedUdpFlowConfig,
    ShadowsocksManagedUdpFlowPlan, ShadowsocksManagedUdpPacketPathCarrierDescriptor,
    ShadowsocksManagedUdpPacketPathDatagramSourceBuild, ShadowsocksManagedUdpPacketPathPlan,
    ShadowsocksOutboundOptionsRef, ShadowsocksTransportLeaf,
};

impl ShadowsocksTransportLeaf {
    pub fn from_options_refs(
        tag: &str,
        server: &str,
        port: u16,
        options: ShadowsocksOutboundOptionsRef<'_>,
    ) -> Self {
        Self::new(tag, server, port, options.cipher, options.password)
    }

    pub fn new(
        tag: impl Into<String>,
        server: impl Into<String>,
        port: u16,
        cipher: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        Self {
            tag: tag.into(),
            server: server.into(),
            port,
            cipher: cipher.into(),
            password: password.into(),
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

    pub fn cipher(&self) -> &str {
        &self.cipher
    }

    pub fn password(&self) -> &str {
        &self.password
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

    pub fn udp_flow_plan(&self) -> Result<ShadowsocksManagedUdpFlowPlan, zero_core::Error> {
        Ok(ShadowsocksManagedUdpFlowPlan::new(
            self.tag.clone(),
            self.server.clone(),
            self.port,
            self.flow_resume()?,
        ))
    }

    pub fn udp_packet_path_plan(
        &self,
    ) -> Result<ShadowsocksManagedUdpPacketPathPlan, zero_core::Error> {
        Ok(ShadowsocksManagedUdpPacketPathPlan::new(
            self.server.clone(),
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
    ) -> Result<(TcpRelayStream, StreamTraffic), RuntimeError>
    where
        OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
        OpenSocketFut: Future<Output = Result<TokioSocket, E>> + Send,
        E: Into<RuntimeError>,
    {
        let upstream = open_socket(&self.server, self.port)
            .await
            .map_err(Into::into)?;
        let metered = MeteredStream::new(TcpRelayStream::from(upstream));
        establish_shadowsocks_tcp_connect(metered, session, &self.cipher, &self.password).await
    }

    pub async fn open_tcp_relay_hop(
        &self,
        stream: TcpRelayStream,
        session: &Session,
    ) -> Result<TcpRelayStream, RuntimeError> {
        apply_shadowsocks_tcp_relay_hop(stream, session, &self.cipher, &self.password).await
    }

    fn flow_config(&self) -> ShadowsocksManagedUdpFlowConfig<'_> {
        ShadowsocksManagedUdpFlowConfig::new(
            &self.tag,
            &self.server,
            self.port,
            &self.cipher,
            &self.password,
        )
    }
}
