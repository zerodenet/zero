use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::managed::flow::ManagedUdpFlowResume;

use super::send::{ManagedExistingSend, ManagedRelaySend};

#[async_trait::async_trait]
pub(crate) trait ManagedDatagramFlowHandler: Send + Sync {
    fn supports_managed_existing(&self, resume: &ManagedUdpFlowResume) -> bool;

    async fn send_managed_existing(
        &mut self,
        request: ManagedExistingSend<'_>,
    ) -> Result<usize, FlowFailure>;
}

#[async_trait::async_trait]
pub(crate) trait ManagedStreamFlowHandler: Send + Sync {
    fn supports_managed_existing(&self, resume: &ManagedUdpFlowResume) -> bool;

    fn supports_managed_relay_existing(&self, resume: &ManagedUdpFlowResume) -> bool;

    async fn send_managed_existing(
        &mut self,
        request: ManagedExistingSend<'_>,
    ) -> Result<usize, FlowFailure>;

    async fn send_managed_relay_existing(
        &mut self,
        request: ManagedRelaySend<'_>,
    ) -> Result<usize, FlowFailure>;
}
