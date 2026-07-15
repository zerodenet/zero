use zero_transport::managed_udp::ProtocolManagedStreamFlowStages;
#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
use zero_transport::managed_udp::ProtocolManagedStreamUdpResumeMetadata;

use super::super::super::model::ManagedStreamHandlerPair;
use super::super::super::stream_manager::{
    ManagedStreamFlowConnector, ManagedStreamFlowManager, SharedManagedStreamFlowManager,
};

pub(crate) type ManagedStreamStages = ProtocolManagedStreamFlowStages;

pub(crate) fn managed_stream_handler_box<T>(stages: ManagedStreamStages) -> ManagedStreamHandlerPair
where
    T: ManagedStreamFlowConnector,
{
    let shared = SharedManagedStreamFlowManager::new(ManagedStreamFlowManager::<T>::new(
        stages.establish_stage,
        stages.relay_upstream_stage,
        stages.relay_establish_stage,
        stages.relay_send_stage,
        stages.mismatch_stage,
        stages.mismatch_message,
    ));
    ManagedStreamHandlerPair {
        stream_packet: Box::new(shared.clone()),
        relay: Box::new(shared),
    }
}

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
pub(crate) fn managed_stream_udp_handler_for_resume<TResume>() -> ManagedStreamHandlerPair
where
    TResume: ProtocolManagedStreamUdpResumeMetadata + ManagedStreamFlowConnector,
{
    managed_stream_handler_box::<TResume>(ManagedStreamStages::from_resume::<TResume>())
}
