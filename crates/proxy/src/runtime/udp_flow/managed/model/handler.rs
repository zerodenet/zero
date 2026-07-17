use crate::runtime::udp_flow::managed::flow::ManagedUdpFlowResume;
use crate::runtime::udp_flow::result::FlowFailure;

#[cfg(feature = "managed-datagram-runtime")]
use super::send::ManagedDatagramExistingSend;
#[cfg(feature = "managed-stream-runtime")]
use super::send::ManagedRelayExistingSend;
#[cfg(feature = "managed-stream-runtime")]
use super::send::ManagedStreamExistingSend;

#[cfg(feature = "managed-datagram-runtime")]
#[async_trait::async_trait]
pub(crate) trait ManagedDatagramFlowHandler: Send + Sync {
    fn supports_managed_existing(&self, resume: &ManagedUdpFlowResume) -> bool;

    async fn send_managed_existing(
        &mut self,
        request: ManagedDatagramExistingSend<'_>,
    ) -> Result<usize, FlowFailure>;
}

#[cfg(feature = "managed-stream-runtime")]
#[async_trait::async_trait]
pub(crate) trait ManagedStreamPacketFlowHandler: Send + Sync {
    fn supports_managed_existing(&self, resume: &ManagedUdpFlowResume) -> bool;

    async fn send_managed_existing(
        &mut self,
        request: ManagedStreamExistingSend<'_>,
    ) -> Result<usize, FlowFailure>;
}

#[cfg(feature = "managed-stream-runtime")]
#[async_trait::async_trait]
pub(crate) trait ManagedRelayFlowHandler: Send + Sync {
    fn supports_managed_relay_existing(&self, resume: &ManagedUdpFlowResume) -> bool;

    async fn send_managed_relay_existing(
        &mut self,
        request: ManagedRelayExistingSend<'_>,
    ) -> Result<usize, FlowFailure>;
}

#[cfg(feature = "managed-stream-runtime")]

pub(crate) struct ManagedStreamHandlerPair {
    #[cfg(feature = "managed-stream-runtime")]
    pub(crate) stream_packet: Box<dyn ManagedStreamPacketFlowHandler>,
    pub(crate) relay: Box<dyn ManagedRelayFlowHandler>,
}
