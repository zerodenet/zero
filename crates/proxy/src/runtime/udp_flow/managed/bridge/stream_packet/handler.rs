use zero_transport::managed_udp::ProtocolManagedStreamFlowStages;
use zero_transport::managed_udp::ProtocolManagedStreamUdpBridgeHandlerMetadata;

use super::super::super::model::ManagedStreamFlowHandler;
use super::super::super::stream_manager::{ManagedStreamFlowConnector, ManagedStreamFlowManager};

pub(crate) type ManagedStreamStages = ProtocolManagedStreamFlowStages;

pub(crate) fn managed_stream_handler_box<T>(
    stages: ManagedStreamStages,
) -> Box<dyn ManagedStreamFlowHandler>
where
    T: ManagedStreamFlowConnector,
{
    Box::new(ManagedStreamFlowManager::<T>::new(
        stages.establish_stage,
        stages.relay_upstream_stage,
        stages.relay_establish_stage,
        stages.relay_send_stage,
        stages.mismatch_stage,
        stages.mismatch_message,
    ))
}

pub(crate) fn managed_stream_udp_handler_for_bridge<TBridge>() -> Box<dyn ManagedStreamFlowHandler>
where
    TBridge: ProtocolManagedStreamUdpBridgeHandlerMetadata,
    TBridge::Resume: ManagedStreamFlowConnector,
{
    managed_stream_handler_box::<TBridge::Resume>(TBridge::managed_stream_flow_stages())
}
