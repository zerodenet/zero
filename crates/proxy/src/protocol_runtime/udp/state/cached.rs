use tokio::task::JoinSet;
use zero_core::Address;
use zero_engine::EngineError;

use super::ProtocolUdpState;
use crate::protocol_runtime::udp::ChainTask;
use crate::runtime::Proxy;

impl ProtocolUdpState {
    pub(crate) async fn send_existing_cached_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<u64>, EngineError> {
        if let Some(session_id) = self
            .vless
            .send_existing(chain_tasks, proxy, target, port, payload)
            .await?
        {
            return Ok(Some(session_id));
        }

        #[cfg(feature = "vmess")]
        if let Some(session_id) = self
            .vmess
            .send_existing(chain_tasks, proxy, target, port, payload)
            .await?
        {
            return Ok(Some(session_id));
        }

        Ok(None)
    }
}
