use ::shadowsocks::transport::{
    ShadowsocksInboundBindings, ShadowsocksInboundOptionsRef, ShadowsocksOutboundOptionsRef,
    ShadowsocksTransportLeaf,
};
use zero_config::{InboundConfig, InboundProtocolConfig, OutboundProtocolConfig};
use zero_engine::EngineError;
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};

use crate::adapters::identity::NamedProtocolAdapter;
use crate::protocol_registry::{
    InboundListenerCapability, ManagedUdpHandlerProvider, OutboundLeafClaim, OutboundLeafInput,
    TcpOutboundCapability, UdpFlowCapability, UdpPacketPathCapability,
};
use crate::runtime::path::TcpPathCategory;
use crate::runtime::udp_flow::managed::ManagedDatagramFlowHandler;

#[cfg(feature = "shadowsocks")]
mod inbound;
#[cfg(feature = "shadowsocks")]
mod tcp;
#[cfg(feature = "shadowsocks")]
pub(crate) mod udp;

#[cfg(feature = "shadowsocks")]
#[derive(Debug)]
pub(crate) struct ShadowsocksAdapter;

fn transport_leaf(
    tag: &str,
    protocol: &OutboundProtocolConfig,
) -> Option<ShadowsocksTransportLeaf> {
    let OutboundProtocolConfig::Shadowsocks {
        server,
        port,
        password,
        cipher,
    } = protocol
    else {
        return None;
    };
    Some(ShadowsocksTransportLeaf::from_options_refs(
        tag,
        server,
        *port,
        ShadowsocksOutboundOptionsRef { cipher, password },
    ))
}

#[cfg(feature = "shadowsocks")]
impl NamedProtocolAdapter for ShadowsocksAdapter {
    const PROTOCOL_NAME: &'static str = "shadowsocks";
    const FEATURE_NAME: &'static str = "shadowsocks";
}

#[cfg(feature = "shadowsocks")]
impl ShadowsocksAdapter {
    pub(crate) fn claim_outbound_leaf_impl<'a>(
        &self,
        input: OutboundLeafInput<'a>,
    ) -> Option<OutboundLeafClaim<'a>> {
        let OutboundLeafInput::Proxy { outbound, .. } = input else {
            return None;
        };
        let leaf = transport_leaf(outbound.tag(), &outbound.protocol)?;
        let tcp = self.claim_tcp_outbound_leaf_impl(leaf.clone());
        Some(OutboundLeafClaim {
            tcp_path: TcpPathCategory::Session,
            tcp,
            udp: Some(self.claim_udp_flow_leaf_impl(leaf.clone())),
            packet_path: self.claim_udp_packet_path_leaf_impl(leaf),
        })
    }
}

#[cfg(feature = "shadowsocks")]
impl UdpPacketPathCapability for ShadowsocksAdapter {}

#[cfg(feature = "shadowsocks")]
impl UdpFlowCapability for ShadowsocksAdapter {}

#[cfg(feature = "shadowsocks")]
impl ManagedUdpHandlerProvider for ShadowsocksAdapter {
    fn managed_datagram_udp_handler(&self) -> Option<Box<dyn ManagedDatagramFlowHandler>> {
        Some(udp::managed_datagram_handler())
    }
}

#[cfg(feature = "shadowsocks")]
impl InboundListenerCapability for ShadowsocksAdapter {
    fn prepare_inbound_listener(
        &self,
        inbound: InboundConfig,
        _source_dir: Option<&std::path::Path>,
    ) -> Result<
        Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation>,
        EngineError,
    > {
        let bindings = match &inbound.protocol {
            InboundProtocolConfig::Shadowsocks {
                password, cipher, ..
            } => ShadowsocksInboundBindings::from_options_refs(ShadowsocksInboundOptionsRef {
                cipher: cipher.as_str(),
                password: password.as_str(),
            })?,
            _ => {
                return Err(EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "shadowsocks inbound listener received non-shadowsocks inbound config",
                )));
            }
        };
        Ok(inbound::prepare(
            inbound.listen.address,
            inbound.listen.port,
            bindings,
        ))
    }
}

#[cfg(feature = "shadowsocks")]
impl TcpOutboundCapability for ShadowsocksAdapter {}

#[cfg(feature = "shadowsocks")]
impl ProtocolMetadata for ShadowsocksAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        ::shadowsocks::ShadowsocksProtocol.descriptor()
    }
}
