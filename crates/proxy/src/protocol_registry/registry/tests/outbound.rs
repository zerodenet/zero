use zero_engine::ResolvedLeafOutbound;

use crate::runtime::path::TcpPathCategory;

use super::fixtures::{compiled_in_outbound_leaves, outbound_leaf_name};

#[test]
fn compiled_in_outbound_leaf_variants_have_expected_protocol_owners() {
    let registry = crate::register::protocol_registry();

    for (leaf, expected_owners) in compiled_in_outbound_leaves() {
        let owner_count = registry
            .entries
            .iter()
            .filter(|entry| entry.support.name() == leaf.protocol_name())
            .count();
        assert_eq!(
            owner_count,
            expected_owners,
            "{} outbound leaf should have {expected_owners} registered protocol owner(s)",
            outbound_leaf_name(&leaf)
        );

        let claimed = registry.claim_outbound_leaf(leaf.clone());
        assert_eq!(
            claimed.as_ref().map(|claim| claim.has_tcp_capability()).ok(),
            Some(expected_owners == 1),
            "{} claimed outbound lookup should expose runtime facts and optional adapter with the same ownership policy",
            outbound_leaf_name(&leaf)
        );
    }
}

#[test]
fn block_outbound_leaf_is_kernel_fact_not_adapter_protocol() {
    let registry = crate::register::protocol_registry();
    let leaf = ResolvedLeafOutbound::Block {
        tag: Some("blocked"),
    };

    let owner_count = registry
        .entries
        .iter()
        .filter(|entry| entry.support.name() == leaf.protocol_name())
        .count();
    assert_eq!(
        owner_count, 0,
        "block should not have a registered protocol owner"
    );

    let claimed = registry
        .claim_outbound_leaf(leaf.clone())
        .expect("block should still expose claimed runtime facts");
    assert!(
        !claimed.has_tcp_capability(),
        "block should not expose an outbound adapter"
    );

    assert_eq!(claimed.runtime.tcp_path, TcpPathCategory::Block);
    assert_eq!(claimed.runtime.health_tag, None);
    assert_eq!(claimed.runtime.endpoint, None);
    assert_eq!(claimed.runtime.kernel_tag, Some("blocked".to_owned()));
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
#[test]
fn udp_outbound_leaf_lookup_matches_tcp_claim_policy() {
    let registry = crate::register::protocol_registry();

    for (leaf, expected_claims) in compiled_in_outbound_leaves() {
        let claimed = registry.claim_outbound_leaf(leaf.clone());
        assert_eq!(
            claimed
                .as_ref()
                .map(|claim| claim.has_udp_flow_capability())
                .ok(),
            Some(expected_claims == 1),
            "{} claimed udp-flow lookup should follow the same claim policy as tcp outbound lookup",
            outbound_leaf_name(&leaf)
        );
    }
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
#[test]
fn packet_path_leaf_lookup_matches_claim_time_packet_path_projection() {
    let registry = crate::register::protocol_registry();

    for (leaf, _) in compiled_in_outbound_leaves() {
        let expected_packet_path = matches!(
            leaf,
            ResolvedLeafOutbound::Socks5 { .. }
                | ResolvedLeafOutbound::Hysteria2 { .. }
                | ResolvedLeafOutbound::Shadowsocks { .. }
        );
        let claimed = registry.claim_outbound_leaf(leaf.clone());
        assert_eq!(
            claimed
                .as_ref()
                .map(|claim| claim.has_udp_packet_path_capability())
                .ok(),
            Some(expected_packet_path),
            "{} claimed packet-path lookup should only expose adapters with packet-path claim-time projection",
            outbound_leaf_name(&leaf)
        );
    }
}

#[cfg(feature = "socks5")]
#[test]
fn registry_executes_adapter_claimed_tcp_leaf_operations() {
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;

    use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};
    use zero_core::Session;
    use zero_engine::EngineError;
    use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};

    use crate::protocol_catalog::protocol_descriptor;
    use crate::protocol_registry::TcpRuntimeServices;
    use crate::protocol_registry::{
        proxy_leaf_runtime, ClaimedTcpOutboundLeaf, ClaimedUdpFlowLeaf, InboundListenerCapability,
        OutboundLeafClaim, OutboundLeafClaimCapability, OutboundLeafRuntime,
        ProtocolSupportCapability, TcpOutboundCapability, UdpFlowCapability,
        UdpPacketPathCapability,
    };
    use crate::runtime::tcp_dispatch::operation::{
        PreparedTcpConnectOperation, PreparedTcpRelayOperation,
    };
    use crate::runtime::udp_dispatch::operation::{
        DirectUdpFlowOperation, PreparedUdpFlowOperation,
    };
    use crate::runtime::udp_dispatch::FlowFailure;
    use crate::transport::{TcpOutboundFailure, TcpRelayStream};

    struct FakeClaimedLeaf {
        runtime: OutboundLeafRuntime,
    }

    impl<'a> ClaimedTcpOutboundLeaf<'a> for FakeClaimedLeaf {
        fn runtime(&self) -> OutboundLeafRuntime {
            self.runtime.clone()
        }

        fn prepare_tcp_connect(
            &self,
            _source_dir: Option<&std::path::Path>,
        ) -> Result<Box<dyn PreparedTcpConnectOperation + 'a>, TcpOutboundFailure> {
            Ok(Box::new(
                crate::runtime::tcp_dispatch::operation::DirectTcpConnectOperation {
                    tag: "claimed".to_owned(),
                },
            ))
        }

        fn prepare_tcp_relay_hop(
            &self,
            _source_dir: Option<&std::path::Path>,
        ) -> Result<Box<dyn PreparedTcpRelayOperation + 'a>, EngineError> {
            Ok(Box::new(FakeRelayOperation))
        }
    }

    struct FakeClaimedUdpLeaf;

    impl<'a> ClaimedUdpFlowLeaf<'a> for FakeClaimedUdpLeaf {
        fn prepare_udp_flow(
            &self,
            _source_dir: Option<&std::path::Path>,
        ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
            Ok(Box::new(DirectUdpFlowOperation {
                tag: "claimed".to_owned(),
            }))
        }
    }

    struct FakeRelayOperation;

    impl PreparedTcpRelayOperation for FakeRelayOperation {
        fn execute<'a>(
            self: Box<Self>,
            _services: TcpRuntimeServices,
            stream: TcpRelayStream,
            _session: &'a Session,
        ) -> Pin<Box<dyn Future<Output = Result<TcpRelayStream, EngineError>> + Send + 'a>>
        where
            Self: 'a,
        {
            Box::pin(async move { Ok(stream) })
        }
    }

    struct FakeClaimedAdapter;

    impl ProtocolMetadata for FakeClaimedAdapter {
        fn descriptor(&self) -> ProtocolCapabilityDescriptor {
            protocol_descriptor("socks5", "test")
        }
    }

    impl ProtocolSupportCapability for FakeClaimedAdapter {
        fn name(&self) -> &'static str {
            "socks5"
        }

        fn feature_name(&self) -> &'static str {
            "test"
        }

        fn supports_inbound(&self, _config: &InboundProtocolConfig) -> bool {
            false
        }

        fn supports_outbound(&self, _config: &OutboundProtocolConfig) -> bool {
            false
        }

        fn has_inbound(&self) -> bool {
            false
        }

        fn has_outbound(&self) -> bool {
            true
        }
    }

    impl InboundListenerCapability for FakeClaimedAdapter {}

    impl TcpOutboundCapability for FakeClaimedAdapter {
        fn claim_tcp_outbound_leaf<'a>(
            &self,
            leaf: ResolvedLeafOutbound<'a>,
        ) -> Option<Box<dyn ClaimedTcpOutboundLeaf<'a> + 'a>> {
            let runtime = proxy_leaf_runtime(&leaf, TcpPathCategory::Tunnel)?;
            match leaf {
                ResolvedLeafOutbound::Socks5 { .. } => Some(Box::new(FakeClaimedLeaf { runtime })),
                _ => None,
            }
        }
    }

    impl UdpFlowCapability for FakeClaimedAdapter {
        fn claim_udp_flow_leaf<'a>(
            &self,
            leaf: ResolvedLeafOutbound<'a>,
        ) -> Option<Box<dyn ClaimedUdpFlowLeaf<'a> + 'a>> {
            match leaf {
                ResolvedLeafOutbound::Socks5 { .. } => Some(Box::new(FakeClaimedUdpLeaf)),
                _ => None,
            }
        }
    }
    impl UdpPacketPathCapability for FakeClaimedAdapter {}

    impl OutboundLeafClaimCapability for FakeClaimedAdapter {
        fn claim_outbound_leaf<'a>(
            &self,
            leaf: ResolvedLeafOutbound<'a>,
        ) -> Option<OutboundLeafClaim<'a>> {
            let tcp = self.claim_tcp_outbound_leaf(leaf.clone())?;
            Some(OutboundLeafClaim {
                runtime: tcp.runtime(),
                tcp,
                udp: Some(self.claim_udp_flow_leaf(leaf)?),
                packet_path: None,
            })
        }
    }

    let mut registry = super::super::ProtocolRegistry::default();
    registry.register_capability(Arc::new(FakeClaimedAdapter));

    let claimed = registry
        .claim_outbound_leaf(ResolvedLeafOutbound::Socks5 {
            tag: "claimed",
            server: "127.0.0.1",
            port: 1080,
            username: None,
            password: None,
        })
        .expect("claim-time tcp leaf");

    match claimed.prepare_tcp_connect(None) {
        Ok(_) => {}
        Err(_) => panic!("claimed tcp connect operation"),
    }
    match claimed.prepare_tcp_relay_hop(None) {
        Ok(_) => {}
        Err(_) => panic!("claimed tcp relay operation"),
    }
}

#[cfg(feature = "socks5")]
#[test]
fn registry_executes_adapter_claimed_udp_leaf_operations() {
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;

    use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};
    use zero_core::Session;
    use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};

    use crate::protocol_catalog::protocol_descriptor;
    use crate::protocol_registry::{
        proxy_leaf_runtime, ClaimedTcpOutboundLeaf, ClaimedUdpFlowLeaf, InboundListenerCapability,
        OutboundLeafClaim, OutboundLeafClaimCapability, OutboundLeafRuntime,
        ProtocolSupportCapability, TcpOutboundCapability, UdpFlowCapability,
        UdpPacketPathCapability,
    };
    use crate::runtime::tcp_dispatch::operation::{
        DirectTcpConnectOperation, PreparedTcpConnectOperation,
    };
    use crate::runtime::udp_dispatch::operation::PreparedUdpFlowOperation;
    use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
    use crate::transport::{RelayCarrier, TcpOutboundFailure, TcpRelayStream};

    struct FakeClaimedUdpLeaf;

    impl<'a> ClaimedUdpFlowLeaf<'a> for FakeClaimedUdpLeaf {
        fn prepare_udp_flow(
            &self,
            _source_dir: Option<&std::path::Path>,
        ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
            Ok(Box::new(FakeUdpFlowOperation))
        }

        fn prepare_owned_udp_relay_final_hop(
            &self,
            _carrier: RelayCarrier,
            _source_dir: Option<&std::path::Path>,
        ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
            Ok(Box::new(FakeUdpFlowOperation))
        }
    }

    struct FakeClaimedTcpLeaf {
        runtime: OutboundLeafRuntime,
    }

    impl<'a> ClaimedTcpOutboundLeaf<'a> for FakeClaimedTcpLeaf {
        fn runtime(&self) -> OutboundLeafRuntime {
            self.runtime.clone()
        }

        fn prepare_tcp_connect(
            &self,
            _source_dir: Option<&std::path::Path>,
        ) -> Result<Box<dyn PreparedTcpConnectOperation + 'a>, TcpOutboundFailure> {
            Ok(Box::new(DirectTcpConnectOperation {
                tag: "fake-claimed-udp".to_owned(),
            }))
        }
    }

    struct FakeUdpFlowOperation;

    impl PreparedUdpFlowOperation for FakeUdpFlowOperation {
        fn execute<'a>(
            self: Box<Self>,
            _dispatch: &'a mut UdpDispatch,
            _ctx: crate::protocol_registry::UdpAdapterContext<'a>,
            _session: &'a Session,
            _payload: &'a [u8],
        ) -> Pin<Box<dyn Future<Output = Result<FlowStartResult, FlowFailure>> + Send + 'a>>
        where
            Self: 'a,
        {
            Box::pin(async move {
                Ok(FlowStartResult::Blocked {
                    tag: "claimed-udp".to_owned(),
                })
            })
        }
    }

    struct FakeClaimedUdpAdapter;

    impl ProtocolMetadata for FakeClaimedUdpAdapter {
        fn descriptor(&self) -> ProtocolCapabilityDescriptor {
            protocol_descriptor("socks5", "test")
        }
    }

    impl ProtocolSupportCapability for FakeClaimedUdpAdapter {
        fn name(&self) -> &'static str {
            "socks5"
        }

        fn feature_name(&self) -> &'static str {
            "test"
        }

        fn supports_inbound(&self, _config: &InboundProtocolConfig) -> bool {
            false
        }

        fn supports_outbound(&self, _config: &OutboundProtocolConfig) -> bool {
            false
        }

        fn has_inbound(&self) -> bool {
            false
        }

        fn has_outbound(&self) -> bool {
            true
        }
    }

    impl InboundListenerCapability for FakeClaimedUdpAdapter {}

    impl TcpOutboundCapability for FakeClaimedUdpAdapter {
        fn claim_tcp_outbound_leaf<'a>(
            &self,
            leaf: ResolvedLeafOutbound<'a>,
        ) -> Option<Box<dyn ClaimedTcpOutboundLeaf<'a> + 'a>> {
            let runtime = proxy_leaf_runtime(&leaf, TcpPathCategory::Tunnel)?;
            match leaf {
                ResolvedLeafOutbound::Socks5 { .. } => {
                    Some(Box::new(FakeClaimedTcpLeaf { runtime }))
                }
                _ => None,
            }
        }
    }

    impl UdpFlowCapability for FakeClaimedUdpAdapter {
        fn claim_udp_flow_leaf<'a>(
            &self,
            leaf: ResolvedLeafOutbound<'a>,
        ) -> Option<Box<dyn ClaimedUdpFlowLeaf<'a> + 'a>> {
            match leaf {
                ResolvedLeafOutbound::Socks5 { .. } => Some(Box::new(FakeClaimedUdpLeaf)),
                _ => None,
            }
        }
    }

    impl UdpPacketPathCapability for FakeClaimedUdpAdapter {}

    impl OutboundLeafClaimCapability for FakeClaimedUdpAdapter {
        fn claim_outbound_leaf<'a>(
            &self,
            leaf: ResolvedLeafOutbound<'a>,
        ) -> Option<OutboundLeafClaim<'a>> {
            let tcp = self.claim_tcp_outbound_leaf(leaf.clone())?;
            Some(OutboundLeafClaim {
                runtime: tcp.runtime(),
                tcp,
                udp: Some(self.claim_udp_flow_leaf(leaf)?),
                packet_path: None,
            })
        }
    }

    let mut registry = super::super::ProtocolRegistry::default();
    registry.register_capability(Arc::new(FakeClaimedUdpAdapter));

    let claimed = registry
        .claim_outbound_leaf(ResolvedLeafOutbound::Socks5 {
            tag: "claimed-udp",
            server: "127.0.0.1",
            port: 1080,
            username: None,
            password: None,
        })
        .expect("claim-time udp leaf");

    match claimed.prepare_udp_flow(None) {
        Ok(_) => {}
        Err(_) => panic!("claimed udp flow operation"),
    }

    let (stream, _peer) = tokio::io::duplex(64);
    match claimed.prepare_owned_udp_relay_final_hop(
        RelayCarrier {
            stream: TcpRelayStream::new(stream),
            server: "claimed-udp.test".to_owned(),
            port: 8443,
        },
        None,
    ) {
        Ok(_) => {}
        Err(_) => panic!("claimed udp relay-final operation"),
    }
}

#[cfg(feature = "socks5")]
#[test]
fn registry_executes_adapter_claimed_udp_packet_path_operations() {
    use std::sync::Arc;

    use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};
    use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};

    use crate::protocol_catalog::protocol_descriptor;
    use crate::protocol_registry::{
        proxy_leaf_runtime, ClaimedTcpOutboundLeaf, ClaimedUdpFlowLeaf, ClaimedUdpPacketPathLeaf,
        InboundListenerCapability, OutboundLeafClaim, OutboundLeafClaimCapability,
        OutboundLeafRuntime, ProtocolSupportCapability, TcpOutboundCapability, UdpFlowCapability,
        UdpPacketPathCapability,
    };
    use crate::runtime::tcp_dispatch::operation::{
        DirectTcpConnectOperation, PreparedTcpConnectOperation,
    };
    use crate::runtime::udp_dispatch::operation::PreparedUdpFlowOperation;
    use crate::runtime::udp_dispatch::packet_path_operation::PreparedUdpPacketPathOperation;
    use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
    use crate::transport::RelayCarrier;
    use crate::transport::TcpOutboundFailure;

    struct FakeClaimedUdpPacketPathLeaf;

    struct FakeClaimedTcpLeaf {
        runtime: OutboundLeafRuntime,
    }

    impl<'a> ClaimedTcpOutboundLeaf<'a> for FakeClaimedTcpLeaf {
        fn runtime(&self) -> OutboundLeafRuntime {
            self.runtime.clone()
        }

        fn prepare_tcp_connect(
            &self,
            _source_dir: Option<&std::path::Path>,
        ) -> Result<Box<dyn PreparedTcpConnectOperation + 'a>, TcpOutboundFailure> {
            Ok(Box::new(DirectTcpConnectOperation {
                tag: "fake-claimed-udp-packet-path".to_owned(),
            }))
        }
    }

    struct FakeClaimedUdpLeaf;

    impl<'a> ClaimedUdpFlowLeaf<'a> for FakeClaimedUdpLeaf {
        fn prepare_udp_flow(
            &self,
            _source_dir: Option<&std::path::Path>,
        ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
            Ok(Box::new(FakeClaimedUdpFlowOperation))
        }

        fn prepare_owned_udp_relay_final_hop(
            &self,
            _carrier: RelayCarrier,
            _source_dir: Option<&std::path::Path>,
        ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
            Ok(Box::new(FakeClaimedUdpFlowOperation))
        }
    }

    impl<'a> ClaimedUdpPacketPathLeaf<'a> for FakeClaimedUdpPacketPathLeaf {
        fn prepare_udp_packet_path(&self) -> Option<Box<dyn PreparedUdpPacketPathOperation + 'a>> {
            Some(Box::new(FakeUdpPacketPathOperation))
        }
    }

    struct FakeUdpPacketPathOperation;

    impl PreparedUdpPacketPathOperation for FakeUdpPacketPathOperation {}

    struct FakeClaimedUdpFlowOperation;

    impl PreparedUdpFlowOperation for FakeClaimedUdpFlowOperation {
        fn execute<'a>(
            self: Box<Self>,
            _dispatch: &'a mut UdpDispatch,
            _ctx: crate::protocol_registry::UdpAdapterContext<'a>,
            _session: &'a zero_core::Session,
            _payload: &'a [u8],
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<FlowStartResult, FlowFailure>> + Send + 'a>,
        >
        where
            Self: 'a,
        {
            Box::pin(async move {
                Ok(FlowStartResult::Blocked {
                    tag: "fake-claimed-udp-packet-path".to_owned(),
                })
            })
        }
    }

    struct FakeClaimedUdpPacketPathAdapter;

    impl ProtocolMetadata for FakeClaimedUdpPacketPathAdapter {
        fn descriptor(&self) -> ProtocolCapabilityDescriptor {
            protocol_descriptor("socks5", "test")
        }
    }

    impl ProtocolSupportCapability for FakeClaimedUdpPacketPathAdapter {
        fn name(&self) -> &'static str {
            "socks5"
        }

        fn feature_name(&self) -> &'static str {
            "test"
        }

        fn supports_inbound(&self, _config: &InboundProtocolConfig) -> bool {
            false
        }

        fn supports_outbound(&self, _config: &OutboundProtocolConfig) -> bool {
            false
        }

        fn has_inbound(&self) -> bool {
            false
        }

        fn has_outbound(&self) -> bool {
            true
        }
    }

    impl InboundListenerCapability for FakeClaimedUdpPacketPathAdapter {}

    impl TcpOutboundCapability for FakeClaimedUdpPacketPathAdapter {
        fn claim_tcp_outbound_leaf<'a>(
            &self,
            leaf: ResolvedLeafOutbound<'a>,
        ) -> Option<Box<dyn ClaimedTcpOutboundLeaf<'a> + 'a>> {
            let runtime = proxy_leaf_runtime(&leaf, TcpPathCategory::Tunnel)?;
            match leaf {
                ResolvedLeafOutbound::Socks5 { .. } => {
                    Some(Box::new(FakeClaimedTcpLeaf { runtime }))
                }
                _ => None,
            }
        }
    }

    impl UdpFlowCapability for FakeClaimedUdpPacketPathAdapter {
        fn claim_udp_flow_leaf<'a>(
            &self,
            leaf: ResolvedLeafOutbound<'a>,
        ) -> Option<Box<dyn ClaimedUdpFlowLeaf<'a> + 'a>> {
            match leaf {
                ResolvedLeafOutbound::Socks5 { .. } => Some(Box::new(FakeClaimedUdpLeaf)),
                _ => None,
            }
        }
    }

    impl UdpPacketPathCapability for FakeClaimedUdpPacketPathAdapter {
        fn claim_udp_packet_path_leaf<'a>(
            &self,
            leaf: ResolvedLeafOutbound<'a>,
        ) -> Option<Box<dyn ClaimedUdpPacketPathLeaf<'a> + 'a>> {
            match leaf {
                ResolvedLeafOutbound::Socks5 { .. } => Some(Box::new(FakeClaimedUdpPacketPathLeaf)),
                _ => None,
            }
        }
    }

    impl OutboundLeafClaimCapability for FakeClaimedUdpPacketPathAdapter {
        fn claim_outbound_leaf<'a>(
            &self,
            leaf: ResolvedLeafOutbound<'a>,
        ) -> Option<OutboundLeafClaim<'a>> {
            let tcp = self.claim_tcp_outbound_leaf(leaf.clone())?;
            Some(OutboundLeafClaim {
                runtime: tcp.runtime(),
                tcp,
                udp: Some(self.claim_udp_flow_leaf(leaf.clone())?),
                packet_path: Some(self.claim_udp_packet_path_leaf(leaf)?),
            })
        }
    }

    let mut registry = super::super::ProtocolRegistry::default();
    registry.register_capability(Arc::new(FakeClaimedUdpPacketPathAdapter));

    let claimed = registry
        .claim_outbound_leaf(ResolvedLeafOutbound::Socks5 {
            tag: "claimed-udp-packet-path",
            server: "127.0.0.1",
            port: 1080,
            username: None,
            password: None,
        })
        .expect("claim-time udp packet-path leaf");

    assert!(
        claimed.prepare_udp_packet_path().is_some(),
        "claimed packet-path leaf should prepare without falling back to raw-leaf callbacks"
    );
}
