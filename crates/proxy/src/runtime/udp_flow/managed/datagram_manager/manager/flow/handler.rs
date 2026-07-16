use std::any::Any;

use async_trait::async_trait;

use super::super::super::super::flow::ManagedUdpFlowResume;
use super::super::super::super::model::ManagedDatagramFlowHandler;
use super::super::super::connector::ManagedDatagramFlowConnector;
use super::super::model::ManagedDatagramFlowManager;
use crate::runtime::udp_flow::managed::model::ManagedDatagramExistingSend;
use crate::runtime::udp_flow::result::FlowFailure;

#[async_trait]
impl<T, C> ManagedDatagramFlowHandler for ManagedDatagramFlowManager<T, C>
where
    T: Any + Clone + Send + Sync + std::fmt::Debug + 'static,
    C: ManagedDatagramFlowConnector<T>,
{
    fn supports_managed_existing(&self, resume: &ManagedUdpFlowResume) -> bool {
        ManagedDatagramFlowManager::supports_managed_existing(self, resume)
    }

    async fn send_managed_existing(
        &mut self,
        request: ManagedDatagramExistingSend<'_>,
    ) -> Result<usize, FlowFailure> {
        ManagedDatagramFlowManager::send_managed_existing(self, request).await
    }
}
