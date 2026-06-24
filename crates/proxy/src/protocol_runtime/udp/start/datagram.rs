use tokio::task::JoinSet;
use zero_core::Session;

#[cfg(feature = "hysteria2")]
use super::super::h2_manager::model::H2SendExisting;
#[cfg(feature = "shadowsocks")]
use super::super::ss_manager::model::SsSendExisting;
use super::super::state::ProtocolUdpState;
#[cfg(feature = "shadowsocks")]
use super::super::ShadowsocksUdpFlow;
use super::super::{ChainTask, FlowFailure};

impl ProtocolUdpState {
    #[cfg(feature = "shadowsocks")]
    pub(crate) async fn start_shadowsocks_udp_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: ShadowsocksUdpFlow<'_>,
    ) -> Result<usize, FlowFailure> {
        self.shadowsocks
            .send_existing(SsSendExisting {
                chain_tasks,
                session_id: flow.session.id,
                proxy: flow.proxy,
                server: flow.server,
                port: flow.port,
                password: flow.password,
                cipher: flow.cipher,
                target: &flow.session.target,
                target_port: flow.session.port,
                payload: flow.payload,
            })
            .await
    }

    #[cfg(feature = "hysteria2")]
    pub(crate) async fn start_hysteria2_udp_flow(
        &mut self,
        request: Hysteria2UdpFlowRequest<'_>,
    ) -> Result<usize, FlowFailure> {
        self.hysteria2.send_existing(request.into_existing()).await
    }
}

#[cfg(feature = "hysteria2")]
pub(crate) struct Hysteria2UdpFlowRequest<'a> {
    pub chain_tasks: &'a mut JoinSet<ChainTask>,
    pub session: &'a Session,
    pub server: &'a str,
    pub port: u16,
    pub password: &'a str,
    pub client_fingerprint: Option<&'a str>,
    pub payload: &'a [u8],
}

#[cfg(feature = "hysteria2")]
impl<'a> Hysteria2UdpFlowRequest<'a> {
    fn into_existing(self) -> H2SendExisting<'a> {
        H2SendExisting {
            chain_tasks: self.chain_tasks,
            session_id: self.session.id,
            server: self.server,
            port: self.port,
            password: self.password,
            client_fingerprint: self.client_fingerprint,
            target: &self.session.target,
            target_port: self.session.port,
            payload: self.payload,
        }
    }
}
