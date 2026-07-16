use async_trait::async_trait;

use super::super::super::model::SharedManagedStreamFlowManager;
use crate::runtime::udp_flow::managed::model::{ManagedRelayExistingSend, ManagedRelayFlowHandler};
use crate::runtime::udp_flow::managed::stream_manager::ManagedStreamFlowConnector;
use crate::runtime::udp_flow::managed::ManagedUdpFlowResume;
use crate::runtime::udp_flow::result::FlowFailure;

#[async_trait]
impl<T> ManagedRelayFlowHandler for SharedManagedStreamFlowManager<T>
where
    T: ManagedStreamFlowConnector,
{
    fn supports_managed_relay_existing(&self, resume: &ManagedUdpFlowResume) -> bool {
        resume.as_ref::<T>().is_some()
    }

    async fn send_managed_relay_existing(
        &mut self,
        request: ManagedRelayExistingSend<'_>,
    ) -> Result<usize, FlowFailure> {
        self.0
            .lock()
            .await
            .send_managed_relay_existing(request)
            .await
    }
}
