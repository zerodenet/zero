use ::socks5::transport::Socks5ManagedUdpPacketPathPlan;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::socks5::Socks5Adapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::udp_dispatch::packet_path_operation::PreparedUdpPacketPathOperation;
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::registered::UpstreamAssociationHandler;

mod handler;
mod packet_path;
mod upstream_association;

struct Socks5PacketPathOperation {
    plan: Socks5ManagedUdpPacketPathPlan,
}

impl PreparedUdpPacketPathOperation for Socks5PacketPathOperation {
    fn carrier_descriptor(
        &self,
    ) -> Option<crate::runtime::udp_flow::packet_path::PacketPathCarrierDescriptor> {
        Some(packet_path::carrier_descriptor(self.plan.clone()))
    }

    fn build_carrier<'a>(
        self: Box<Self>,
        ctx: crate::protocol_registry::UdpAdapterContext<'a>,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<
                        std::sync::Arc<
                            dyn crate::runtime::udp_flow::packet_path::PacketPathCarrier,
                        >,
                        EngineError,
                    >,
                > + Send
                + 'a,
        >,
    >
    where
        Self: 'a,
    {
        Box::pin(async move { packet_path::build(ctx.runtime_services(), self.plan).await })
    }
}

pub(crate) fn upstream_association_handler() -> Box<dyn UpstreamAssociationHandler> {
    handler::upstream_association_handler()
}

impl Socks5Adapter {
    pub(super) fn prepare_udp_packet_path_impl<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<Box<dyn PreparedUdpPacketPathOperation + 'a>> {
        let leaf = super::transport_leaf(&leaf)?;
        Some(Box::new(Socks5PacketPathOperation {
            plan: leaf.udp_packet_path_plan(),
        }))
    }

    pub(super) fn prepare_udp_flow_impl<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Result<
        Box<dyn crate::runtime::udp_dispatch::operation::PreparedUdpFlowOperation + 'a>,
        FlowFailure,
    > {
        let Some(leaf) = super::transport_leaf(&leaf) else {
            return Err(FlowFailure {
                stage: "udp_unsupported_leaf",
                error: EngineError::Io(std::io::Error::other(format!(
                    "{} adapter received unsupported UDP leaf: {leaf:?}",
                    self.name()
                ))),
                upstream: None,
            });
        };
        let (tag, server, port, resume) = leaf.udp_flow_plan().into_parts();
        Ok(Box::new(
            crate::runtime::udp_dispatch::operation::RegisteredAssociationUdpOperation {
                tag,
                server,
                port,
                resume,
            },
        ))
    }
}
