use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};
use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};

use crate::protocol_catalog::protocol_descriptor;
use crate::protocol_registry::{
    InboundListenerCapability, OutboundAdapterContext, ProtocolSupportCapability,
    TcpOutboundCapability, UdpFlowCapability, UdpPacketPathCapability,
};
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure, TcpRelayStream};

#[derive(Debug, Default)]
pub(super) struct TcpCapabilityCalls {
    connects: AtomicUsize,
    relay_hops: AtomicUsize,
    udp_starts: AtomicUsize,
    udp_two_stream_starts: AtomicUsize,
    udp_final_hop_starts: AtomicUsize,
    udp_payload_bytes: AtomicUsize,
    udp_final_hop_port: AtomicUsize,
    fail_udp: AtomicBool,
    fail_tcp: AtomicBool,
    fail_relay: AtomicBool,
    fail_bind: AtomicBool,
    inbound_binds: AtomicUsize,
    inbound_spawns: AtomicUsize,
    packet_descriptors: AtomicUsize,
    packet_sources: AtomicUsize,
    packet_builds: AtomicUsize,
    packet_sends: AtomicUsize,
    reloads: AtomicUsize,
    provider_forwards: AtomicUsize,
    provider_generation: AtomicUsize,
    upstream_provider_sends: AtomicUsize,
}

impl TcpCapabilityCalls {
    pub(super) fn connects(&self) -> usize {
        self.connects.load(Ordering::SeqCst)
    }

    pub(super) fn relay_hops(&self) -> usize {
        self.relay_hops.load(Ordering::SeqCst)
    }

    pub(super) fn udp_starts(&self) -> usize {
        self.udp_starts.load(Ordering::SeqCst)
    }

    pub(super) fn udp_two_stream_starts(&self) -> usize {
        self.udp_two_stream_starts.load(Ordering::SeqCst)
    }

    pub(super) fn udp_final_hop_starts(&self) -> usize {
        self.udp_final_hop_starts.load(Ordering::SeqCst)
    }

    pub(super) fn udp_final_hop_port(&self) -> usize {
        self.udp_final_hop_port.load(Ordering::SeqCst)
    }

    pub(super) fn udp_payload_bytes(&self) -> usize {
        self.udp_payload_bytes.load(Ordering::SeqCst)
    }

    pub(super) fn set_fail_udp(&self, fail: bool) {
        self.fail_udp.store(fail, Ordering::SeqCst);
    }

    pub(super) fn set_fail_tcp(&self, fail: bool) {
        self.fail_tcp.store(fail, Ordering::SeqCst);
    }

    pub(super) fn set_fail_relay(&self, fail: bool) {
        self.fail_relay.store(fail, Ordering::SeqCst);
    }

    pub(super) fn set_fail_bind(&self, fail: bool) {
        self.fail_bind.store(fail, Ordering::SeqCst);
    }

    pub(super) fn inbound_binds(&self) -> usize {
        self.inbound_binds.load(Ordering::SeqCst)
    }

    pub(super) fn inbound_spawns(&self) -> usize {
        self.inbound_spawns.load(Ordering::SeqCst)
    }

    pub(super) fn packet_descriptors(&self) -> usize {
        self.packet_descriptors.load(Ordering::SeqCst)
    }

    pub(super) fn packet_sources(&self) -> usize {
        self.packet_sources.load(Ordering::SeqCst)
    }

    pub(super) fn packet_builds(&self) -> usize {
        self.packet_builds.load(Ordering::SeqCst)
    }

    pub(super) fn packet_sends(&self) -> usize {
        self.packet_sends.load(Ordering::SeqCst)
    }

    pub(super) fn reloads(&self) -> usize {
        self.reloads.load(Ordering::SeqCst)
    }

    pub(super) fn provider_forwards(&self) -> usize {
        self.provider_forwards.load(Ordering::SeqCst)
    }

    pub(super) fn provider_generation(&self) -> usize {
        self.provider_generation.load(Ordering::SeqCst)
    }

    pub(super) fn upstream_provider_sends(&self) -> usize {
        self.upstream_provider_sends.load(Ordering::SeqCst)
    }
}

#[cfg(feature = "socks5")]
#[derive(Debug)]
pub(super) struct FakeUpstreamResume;

#[cfg(feature = "socks5")]
struct FakeUpstreamHandler {
    calls: Arc<TcpCapabilityCalls>,
}

#[cfg(feature = "socks5")]
#[async_trait]
impl crate::runtime::udp_flow::registered::UpstreamAssociationHandler for FakeUpstreamHandler {
    fn supports_upstream_resume(
        &self,
        resume: &crate::runtime::udp_flow::managed::ManagedUdpFlowResume,
    ) -> bool {
        resume.as_ref::<FakeUpstreamResume>().is_some()
    }

    async fn send_upstream(
        &mut self,
        inbound_tag: &str,
        request: crate::runtime::udp_flow::registered::UpstreamAssociationSend<'_>,
    ) -> Result<usize, FlowFailure> {
        assert_eq!(inbound_tag, "fake-inbound");
        assert!(request.resume.as_ref::<FakeUpstreamResume>().is_some());
        self.calls
            .upstream_provider_sends
            .fetch_add(1, Ordering::SeqCst);
        Ok(request.payload.len())
    }

    async fn recv_upstream_response(
        &self,
        _: &mut [u8],
    ) -> Result<crate::runtime::udp_flow::response::UpstreamUdpResponse, EngineError> {
        Err(EngineError::Io(std::io::Error::other(
            "fake upstream has no response",
        )))
    }

    fn upstream_outbound_tag(&self) -> Option<&str> {
        None
    }

    fn upstream_idle_deadline(&self) -> Option<tokio::time::Instant> {
        None
    }

    fn touch_upstream_idle(&mut self, _: std::time::Duration) {}

    fn drop_upstream_association(&mut self) -> Option<(String, String, u16)> {
        None
    }

    fn close_idle_upstream(&mut self) -> Option<(String, String, u16)> {
        None
    }

    fn close_all_upstreams(&mut self) {}
}

#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
#[derive(Debug)]
pub(super) struct FakeProviderResume {
    pub(super) generation: usize,
}

#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
struct FakeProviderDatagramHandler {
    calls: Arc<TcpCapabilityCalls>,
}

#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
#[async_trait]
impl crate::runtime::udp_flow::managed::ManagedDatagramFlowHandler for FakeProviderDatagramHandler {
    fn supports_managed_existing(
        &self,
        resume: &crate::runtime::udp_flow::managed::ManagedUdpFlowResume,
    ) -> bool {
        resume
            .as_ref::<FakeProviderResume>()
            .is_some_and(|resume| resume.generation == self.calls.provider_generation())
    }

    async fn send_managed_existing(
        &mut self,
        request: crate::runtime::udp_flow::managed::model::ManagedDatagramExistingSend<'_>,
    ) -> Result<usize, FlowFailure> {
        assert!(request.resume.as_ref::<FakeProviderResume>().is_some());
        self.calls.provider_forwards.fetch_add(1, Ordering::SeqCst);
        Ok(request.payload.len())
    }
}

#[derive(Debug)]
pub(super) struct FakeTcpCapability {
    calls: Arc<TcpCapabilityCalls>,
}

impl FakeTcpCapability {
    pub(super) fn new(calls: Arc<TcpCapabilityCalls>) -> Self {
        Self { calls }
    }
}

impl ProtocolMetadata for FakeTcpCapability {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        protocol_descriptor("fake-tcp", "test")
    }
}

impl ProtocolSupportCapability for FakeTcpCapability {
    fn name(&self) -> &'static str {
        "fake-tcp"
    }

    fn feature_name(&self) -> &'static str {
        "test"
    }

    fn supports_inbound(&self, config: &InboundProtocolConfig) -> bool {
        config.protocol_name() == "direct"
    }

    fn supports_outbound(&self, _: &OutboundProtocolConfig) -> bool {
        false
    }

    fn has_inbound(&self) -> bool {
        true
    }

    fn has_outbound(&self) -> bool {
        true
    }

    fn on_config_reloaded(&self) {
        self.calls.reloads.fetch_add(1, Ordering::SeqCst);
        self.calls
            .provider_generation
            .fetch_add(1, Ordering::SeqCst);
    }
}

#[async_trait]
impl InboundListenerCapability for FakeTcpCapability {
    async fn bind_inbound(
        &self,
        _: &zero_config::InboundConfig,
        _: Option<&std::path::Path>,
    ) -> Result<crate::protocol_registry::BoundInbound, EngineError> {
        self.calls.inbound_binds.fetch_add(1, Ordering::SeqCst);
        if self.calls.fail_bind.load(Ordering::SeqCst) {
            return Err(EngineError::Io(std::io::Error::other(
                "fake inbound bind failure",
            )));
        }
        let listener = zero_platform_tokio::TokioListener::bind("127.0.0.1:0")
            .await
            .map_err(EngineError::Io)?;
        Ok(crate::protocol_registry::BoundInbound::Tcp(listener))
    }

    fn spawn_inbound(
        &self,
        _: crate::protocol_registry::InboundAdapterContext<'_>,
        _: zero_config::InboundConfig,
        bound: crate::protocol_registry::BoundInbound,
        _: tokio::sync::watch::Receiver<bool>,
        _: &mut tokio::task::JoinSet<Result<(), EngineError>>,
    ) {
        self.calls.inbound_spawns.fetch_add(1, Ordering::SeqCst);
        drop(bound);
    }
}

#[cfg(feature = "socks5")]
impl crate::protocol_registry::UpstreamUdpHandlerProvider for FakeTcpCapability {
    fn upstream_association_handler(
        &self,
    ) -> Box<dyn crate::runtime::udp_flow::registered::UpstreamAssociationHandler> {
        Box::new(FakeUpstreamHandler {
            calls: self.calls.clone(),
        })
    }
}

#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
impl crate::protocol_registry::ManagedUdpHandlerProvider for FakeTcpCapability {
    #[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
    fn managed_datagram_udp_handler(
        &self,
    ) -> Option<Box<dyn crate::runtime::udp_flow::managed::ManagedDatagramFlowHandler>> {
        Some(Box::new(FakeProviderDatagramHandler {
            calls: self.calls.clone(),
        }))
    }
}
#[async_trait]
impl UdpFlowCapability for FakeTcpCapability {
    async fn start_udp_flow(
        &self,
        _: &mut UdpDispatch,
        _: crate::protocol_registry::UdpAdapterContext<'_>,
        _: &Session,
        _: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        self.calls.udp_starts.fetch_add(1, Ordering::SeqCst);
        self.calls
            .udp_payload_bytes
            .fetch_add(payload.len(), Ordering::SeqCst);
        if self.calls.fail_udp.load(Ordering::SeqCst) {
            return Err(FlowFailure {
                stage: "fake_udp_start",
                error: EngineError::Io(std::io::Error::other("fake udp failure")),
                upstream: Some(("fake-upstream.test".to_owned(), 5353)),
            });
        }
        Ok(FlowStartResult::Blocked {
            tag: "fake-udp".to_owned(),
        })
    }

    fn udp_relay_needs_two_streams(&self, _: &ResolvedLeafOutbound<'_>) -> bool {
        true
    }

    async fn start_udp_relay_two_stream(
        &self,
        _: &mut UdpDispatch,
        _: crate::protocol_registry::UdpAdapterContext<'_>,
        _: &Session,
        chain: Vec<ResolvedLeafOutbound<'_>>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        assert_eq!(chain.len(), 2);
        self.calls
            .udp_two_stream_starts
            .fetch_add(1, Ordering::SeqCst);
        self.calls
            .udp_payload_bytes
            .fetch_add(payload.len(), Ordering::SeqCst);
        Ok(FlowStartResult::Blocked {
            tag: "fake-two-stream".to_owned(),
        })
    }

    async fn start_udp_relay_final_hop(
        &self,
        _: &mut UdpDispatch,
        _: crate::protocol_registry::UdpAdapterContext<'_>,
        _: &Session,
        carrier: crate::transport::RelayCarrier,
        _: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        self.calls
            .udp_final_hop_starts
            .fetch_add(1, Ordering::SeqCst);
        self.calls
            .udp_final_hop_port
            .store(carrier.port as usize, Ordering::SeqCst);
        self.calls
            .udp_payload_bytes
            .fetch_add(payload.len(), Ordering::SeqCst);
        Ok(FlowStartResult::Blocked {
            tag: "fake-final-hop".to_owned(),
        })
    }
}
struct FakeDatagramCodec;

impl zero_traits::DatagramCodec<zero_core::Address> for FakeDatagramCodec {
    type Error = zero_core::Error;

    fn encode(
        &self,
        _: &zero_core::Address,
        _: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, Self::Error> {
        Ok(payload.to_vec())
    }

    fn decode(&self, _: &[u8]) -> Option<(zero_core::Address, u16, Vec<u8>)> {
        None
    }
}

struct FakePacketPathCarrier {
    calls: Arc<TcpCapabilityCalls>,
}

#[async_trait]
impl crate::runtime::udp_flow::packet_path::PacketPathCarrier for FakePacketPathCarrier {
    async fn send_to(&self, _: &zero_core::Address, _: u16, _: &[u8]) -> Result<(), EngineError> {
        self.calls.packet_sends.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn recv_from(&self, _: &mut [u8]) -> Result<usize, EngineError> {
        Ok(0)
    }
}

#[async_trait]
impl UdpPacketPathCapability for FakeTcpCapability {
    fn udp_packet_path_carrier_descriptor(
        &self,
        _: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::runtime::udp_flow::packet_path::PacketPathCarrierDescriptor> {
        self.calls.packet_descriptors.fetch_add(1, Ordering::SeqCst);
        Some(
            crate::runtime::udp_flow::packet_path::packet_path_carrier_descriptor(
                "fake-carrier-key".to_owned(),
                "carrier.test",
                1443,
            ),
        )
    }

    async fn build_udp_packet_path(
        &self,
        _: crate::protocol_registry::UdpAdapterContext<'_>,
        _: &ResolvedLeafOutbound<'_>,
    ) -> Result<Arc<dyn crate::runtime::udp_flow::packet_path::PacketPathCarrier>, EngineError>
    {
        self.calls.packet_builds.fetch_add(1, Ordering::SeqCst);
        Ok(Arc::new(FakePacketPathCarrier {
            calls: self.calls.clone(),
        }))
    }

    fn udp_datagram_source(
        &self,
        _: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::runtime::udp_flow::packet_path::UdpDatagramSource> {
        self.calls.packet_sources.fetch_add(1, Ordering::SeqCst);
        Some(crate::runtime::udp_flow::packet_path::udp_datagram_source(
            "fake-datagram",
            "datagram.test",
            2443,
            "fake-datagram-key".to_owned(),
            Arc::new(FakeDatagramCodec),
        ))
    }
}

#[async_trait]
impl TcpOutboundCapability for FakeTcpCapability {
    fn claims_outbound_leaf(&self, _: &ResolvedLeafOutbound<'_>) -> bool {
        true
    }

    async fn connect_tcp(
        &self,
        _: OutboundAdapterContext<'_>,
        _: &Session,
        _: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        self.calls.connects.fetch_add(1, Ordering::SeqCst);
        if self.calls.fail_tcp.load(Ordering::SeqCst) {
            return Err(TcpOutboundFailure {
                stage: "fake_tcp_connect",
                error: EngineError::Io(std::io::Error::other("fake TCP failure")),
                upstream_endpoint: Some(("fake-tcp.test".to_owned(), 8443)),
            });
        }
        Ok(EstablishedTcpOutbound::block("fake"))
    }

    async fn apply_relay_hop(
        &self,
        _: OutboundAdapterContext<'_>,
        stream: TcpRelayStream,
        _: &Session,
        _: &ResolvedLeafOutbound<'_>,
    ) -> Result<TcpRelayStream, EngineError> {
        self.calls.relay_hops.fetch_add(1, Ordering::SeqCst);
        if self.calls.fail_relay.load(Ordering::SeqCst) {
            return Err(EngineError::Io(std::io::Error::other("fake relay failure")));
        }
        Ok(stream)
    }
}
