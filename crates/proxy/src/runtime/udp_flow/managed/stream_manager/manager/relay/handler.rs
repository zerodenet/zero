use async_trait::async_trait;

use super::super::model::SharedManagedStreamFlowManager;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
use crate::runtime::udp_flow::managed::model::{
    ManagedRelayExistingSend, ManagedRelayFlowHandler, ManagedStreamExistingSend,
    ManagedStreamPacketFlowHandler,
};
use crate::runtime::udp_flow::managed::ManagedUdpFlowResume;
use crate::runtime::udp_flow::result::FlowFailure;
use crate::runtime::udp_flow::managed::stream_manager::ManagedStreamFlowConnector;

#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
#[async_trait]
impl<T> ManagedStreamPacketFlowHandler for SharedManagedStreamFlowManager<T>
where
    T: ManagedStreamFlowConnector,
{
    fn supports_managed_existing(&self, resume: &ManagedUdpFlowResume) -> bool {
        resume.as_ref::<T>().is_some()
    }

    async fn send_managed_existing(
        &mut self,
        request: ManagedStreamExistingSend<'_>,
    ) -> Result<usize, FlowFailure> {
        self.0.lock().await.send_managed_existing(request).await
    }
}

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
