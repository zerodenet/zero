#[cfg(feature = "hysteria2")]
use crate::protocol_runtime::udp::h2_manager::H2ChainManager;
#[cfg(feature = "shadowsocks")]
use crate::protocol_runtime::udp::ss_manager::SsChainManager;
use crate::protocol_runtime::udp::state::managed::model::ManagedExistingSend;
use crate::protocol_runtime::udp::{FlowFailure, ProtocolUdpFlowSnapshot};
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;
use crate::runtime::Proxy;
use tokio::task::JoinSet;

use crate::protocol_runtime::udp::flows::ManagedDatagramFlow;

pub(in crate::protocol_runtime::udp::state::managed) struct ManagedDatagramState {
    #[cfg(feature = "shadowsocks")]
    shadowsocks: SsChainManager,
    #[cfg(feature = "hysteria2")]
    hysteria2: H2ChainManager,
}

impl ManagedDatagramState {
    pub(in crate::protocol_runtime::udp::state::managed) fn new() -> Self {
        Self {
            #[cfg(feature = "shadowsocks")]
            shadowsocks: SsChainManager::new(),
            #[cfg(feature = "hysteria2")]
            hysteria2: H2ChainManager::new(),
        }
    }

    pub(in crate::protocol_runtime::udp::state::managed) async fn start_datagram_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: ManagedDatagramFlow<'_>,
    ) -> Option<Result<usize, FlowFailure>> {
        #[cfg(feature = "shadowsocks")]
        if self.shadowsocks.supports_managed_existing(&flow.resume) {
            return Some(
                self.shadowsocks
                    .send_managed_existing(ManagedExistingSend::datagram(chain_tasks, &flow))
                    .await,
            );
        }
        #[cfg(feature = "hysteria2")]
        if self.hysteria2.supports_managed_existing(&flow.resume) {
            return Some(
                self.hysteria2
                    .send_managed_existing(ManagedExistingSend::datagram(chain_tasks, &flow))
                    .await,
            );
        }
        None
    }

    pub(in crate::protocol_runtime::udp::state::managed) async fn forward_existing_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        flow: &UdpFlowSnapshot,
        snapshot: &ProtocolUdpFlowSnapshot,
        payload: &[u8],
    ) -> Option<Result<usize, FlowFailure>> {
        let upstream = flow
            .outbound
            .upstream()
            .expect("protocol flow should have upstream");
        let resume = snapshot.resume();
        #[cfg(feature = "shadowsocks")]
        if self.shadowsocks.supports_managed_existing(resume) {
            return Some(
                self.shadowsocks
                    .send_managed_existing(ManagedExistingSend::forwarded(
                        chain_tasks,
                        proxy,
                        flow,
                        resume.clone(),
                        upstream.server,
                        upstream.port,
                        payload,
                    ))
                    .await,
            );
        }
        #[cfg(feature = "hysteria2")]
        if self.hysteria2.supports_managed_existing(resume) {
            return Some(
                self.hysteria2
                    .send_managed_existing(ManagedExistingSend::forwarded(
                        chain_tasks,
                        proxy,
                        flow,
                        resume.clone(),
                        upstream.server,
                        upstream.port,
                        payload,
                    ))
                    .await,
            );
        }
        None
    }
}
