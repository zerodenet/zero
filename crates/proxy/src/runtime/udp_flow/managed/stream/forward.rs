use super::super::model::ManagedStreamExistingSend;
use super::model::ManagedStreamState;
use crate::runtime::udp_flow::managed::flow::ManagedUdpFlowResume;
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::result::FlowFailure;
use crate::runtime::udp_flow::snapshot::UdpFlowSnapshot;
use crate::runtime::Proxy;
use tokio::task::JoinSet;

impl ManagedStreamState {
    pub(in crate::runtime::udp_flow::managed) async fn forward_existing_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        flow: &UdpFlowSnapshot,
        resume: &ManagedUdpFlowResume,
        payload: &[u8],
    ) -> Option<Result<usize, FlowFailure>> {
        let upstream = flow
            .outbound
            .upstream()
            .expect("protocol flow should have upstream");
        #[cfg(any(
            feature = "vless",
            feature = "vmess",
            feature = "trojan",
            feature = "mieru"
        ))]
        for handler in &mut self.stream_packet_handlers {
            if !handler.supports_managed_existing(resume) {
                continue;
            }
            return Some(
                handler
                    .send_managed_existing(ManagedStreamExistingSend::forwarded(
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
