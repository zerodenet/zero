use super::super::super::model::ManagedStreamHandlerPair;
use super::super::super::stream_manager::{
    ManagedPacketUdpResume, ManagedPacketUdpResumeConnector, ManagedStreamFlowConnector,
    ManagedStreamFlowManager, ManagedTupleUdpResume, ManagedTupleUdpResumeConnector,
    SharedManagedStreamFlowManager,
};

pub(crate) trait ManagedStreamResumeMetadata {
    const ESTABLISH_STAGE: &'static str;
    const RELAY_UPSTREAM_STAGE: &'static str;
    const RELAY_ESTABLISH_STAGE: &'static str;
    const RELAY_SEND_STAGE: &'static str;
    const MISMATCH_STAGE: &'static str;
    const MISMATCH_MESSAGE: &'static str;
}

impl<T> ManagedStreamResumeMetadata for ManagedTupleUdpResume<T>
where
    T: ManagedTupleUdpResumeConnector,
{
    const ESTABLISH_STAGE: &'static str = T::ESTABLISH_STAGE;
    const RELAY_UPSTREAM_STAGE: &'static str = T::RELAY_UPSTREAM_STAGE;
    const RELAY_ESTABLISH_STAGE: &'static str = T::RELAY_ESTABLISH_STAGE;
    const RELAY_SEND_STAGE: &'static str = T::RELAY_SEND_STAGE;
    const MISMATCH_STAGE: &'static str = T::MISMATCH_STAGE;
    const MISMATCH_MESSAGE: &'static str = T::MISMATCH_MESSAGE;
}

impl<T> ManagedStreamResumeMetadata for ManagedPacketUdpResume<T>
where
    T: ManagedPacketUdpResumeConnector,
{
    const ESTABLISH_STAGE: &'static str = T::ESTABLISH_STAGE;
    const RELAY_UPSTREAM_STAGE: &'static str = T::RELAY_UPSTREAM_STAGE;
    const RELAY_ESTABLISH_STAGE: &'static str = T::RELAY_ESTABLISH_STAGE;
    const RELAY_SEND_STAGE: &'static str = T::RELAY_SEND_STAGE;
    const MISMATCH_STAGE: &'static str = T::MISMATCH_STAGE;
    const MISMATCH_MESSAGE: &'static str = T::MISMATCH_MESSAGE;
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ManagedStreamStages {
    pub(crate) establish_stage: &'static str,
    pub(crate) relay_upstream_stage: &'static str,
    pub(crate) relay_establish_stage: &'static str,
    pub(crate) relay_send_stage: &'static str,
    pub(crate) mismatch_stage: &'static str,
    pub(crate) mismatch_message: &'static str,
}

impl ManagedStreamStages {
    pub(crate) fn from_resume<T>() -> Self
    where
        T: ManagedStreamResumeMetadata,
    {
        Self {
            establish_stage: T::ESTABLISH_STAGE,
            relay_upstream_stage: T::RELAY_UPSTREAM_STAGE,
            relay_establish_stage: T::RELAY_ESTABLISH_STAGE,
            relay_send_stage: T::RELAY_SEND_STAGE,
            mismatch_stage: T::MISMATCH_STAGE,
            mismatch_message: T::MISMATCH_MESSAGE,
        }
    }
}

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

#[cfg(feature = "managed-stream-runtime")]
pub(crate) fn managed_stream_udp_handler_for_resume<TResume>() -> ManagedStreamHandlerPair
where
    TResume: ManagedStreamResumeMetadata + ManagedStreamFlowConnector,
{
    managed_stream_handler_box::<TResume>(ManagedStreamStages::from_resume::<TResume>())
}
