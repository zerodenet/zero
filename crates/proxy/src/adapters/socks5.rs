use ::socks5::transport::{
    Socks5InboundAcceptor, Socks5InboundUserRef, Socks5OutboundOptionsRef, Socks5TransportLeaf,
};
use zero_config::{InboundConfig, InboundProtocolConfig, OutboundProtocolConfig};
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};

use crate::adapters::identity::{
    named_protocol_supports_inbound, named_protocol_supports_outbound, NamedProtocolAdapter,
};
use crate::protocol_registry::{
    InboundListenerCapability, OutboundLeafClaim, ProtocolSupportCapability, TcpOutboundCapability,
    UdpFlowCapability, UdpPacketPathCapability, UpstreamUdpHandlerProvider,
};
use crate::runtime::udp_flow::registered::UpstreamAssociationHandler;

#[cfg(feature = "socks5")]
pub(super) mod inbound;
#[cfg(feature = "socks5")]
mod tcp;
#[cfg(feature = "socks5")]
pub(crate) mod udp;

#[cfg(feature = "socks5")]
#[derive(Debug)]
pub(crate) struct Socks5Adapter;

fn transport_leaf(leaf: &ResolvedLeafOutbound<'_>) -> Option<Socks5TransportLeaf> {
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
    Some(Socks5TransportLeaf::from_options_refs(
        tag,
        server,
        *port,
        Socks5OutboundOptionsRef {
            username: username.as_deref(),
            password: password.as_deref(),
        },
    ))
}

#[cfg(feature = "socks5")]
impl NamedProtocolAdapter for Socks5Adapter {
    const PROTOCOL_NAME: &'static str = "socks5";
    const FEATURE_NAME: &'static str = "socks5";
}

#[cfg(feature = "socks5")]
impl Socks5Adapter {
    pub(crate) fn claim_outbound_leaf_impl<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<OutboundLeafClaim<'a>> {
        let tcp = self.claim_tcp_outbound_leaf_impl(leaf.clone())?;
        Some(OutboundLeafClaim {
            runtime: tcp.runtime(),
            tcp,
            udp: self.claim_udp_flow_leaf_impl(leaf.clone()),
            packet_path: self.claim_udp_packet_path_leaf_impl(leaf),
        })
    }
}

#[cfg(feature = "socks5")]
impl UdpPacketPathCapability for Socks5Adapter {}

#[cfg(feature = "socks5")]
impl UdpFlowCapability for Socks5Adapter {}

#[cfg(feature = "socks5")]
impl UpstreamUdpHandlerProvider for Socks5Adapter {
    fn upstream_association_handler(&self) -> Box<dyn UpstreamAssociationHandler> {
        udp::upstream_association_handler()
    }
}

#[cfg(feature = "socks5")]
impl InboundListenerCapability for Socks5Adapter {
    fn prepare_inbound_listener(
        &self,
        inbound: InboundConfig,
        _source_dir: Option<&std::path::Path>,
    ) -> Result<
        Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation>,
        EngineError,
    > {
        let acceptor = match &inbound.protocol {
            InboundProtocolConfig::Socks5 { users } => {
                Socks5InboundAcceptor::from_options_refs(users.iter().map(|user| {
                    Socks5InboundUserRef {
                        username: user.username.as_str(),
                        password: user.password.as_str(),
                        principal_key: user.principal_key.as_deref(),
                        up_bps: user.up_bps,
                        down_bps: user.down_bps,
                    }
                }))
            }
            _ => {
                return Err(EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "socks5 inbound listener received non-socks5 inbound config",
                )));
            }
        };
        Ok(inbound::prepare(acceptor))
    }
}

#[cfg(feature = "socks5")]
impl TcpOutboundCapability for Socks5Adapter {}

#[cfg(feature = "socks5")]
impl ProtocolSupportCapability for Socks5Adapter {
    fn name(&self) -> &'static str {
        <Self as NamedProtocolAdapter>::PROTOCOL_NAME
    }

    fn feature_name(&self) -> &'static str {
        <Self as NamedProtocolAdapter>::FEATURE_NAME
    }

    fn has_inbound(&self) -> bool {
        <Self as NamedProtocolAdapter>::HAS_INBOUND
    }

    fn has_outbound(&self) -> bool {
        <Self as NamedProtocolAdapter>::HAS_OUTBOUND
    }

    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        named_protocol_supports_inbound::<Self>(c)
    }

    fn supports_outbound(&self, c: &OutboundProtocolConfig) -> bool {
        named_protocol_supports_outbound::<Self>(c)
    }
}

#[cfg(feature = "socks5")]
impl ProtocolMetadata for Socks5Adapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        ::socks5::Socks5Protocol.descriptor()
    }
}
