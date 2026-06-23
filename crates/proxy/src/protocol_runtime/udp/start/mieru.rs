use tokio::task::JoinSet;
use zero_core::Session;

use super::super::state::ProtocolUdpState;
use super::super::{
    ChainTask, FlowFailure, MieruRelayExisting, MieruSendExisting, MieruUdpRelayFlow,
};
use crate::runtime::Proxy;

impl ProtocolUdpState {
    pub(crate) async fn start_mieru_udp_flow(
        &mut self,
        request: MieruUdpFlowRequest<'_>,
    ) -> Result<usize, FlowFailure> {
        self.mieru.send_existing(request.into_existing()).await
    }

    pub(crate) async fn start_mieru_udp_relay_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: MieruUdpRelayFlow<'_>,
    ) -> Result<usize, FlowFailure> {
        self.mieru
            .send_relay_existing(MieruRelayExisting {
                chain_tasks,
                session_id: flow.session.id,
                stream: flow.carrier.stream,
                server: flow.server,
                port: flow.port,
                username: flow.username,
                password: flow.password,
                target: &flow.session.target,
                target_port: flow.session.port,
                payload: flow.payload,
            })
            .await
    }
}

pub(crate) struct MieruUdpFlowRequest<'a> {
    pub chain_tasks: &'a mut JoinSet<ChainTask>,
    pub proxy: &'a Proxy,
    pub session: &'a Session,
    pub server: &'a str,
    pub port: u16,
    pub username: &'a str,
    pub password: &'a str,
    pub relay_chain: bool,
    pub payload: &'a [u8],
}

impl<'a> MieruUdpFlowRequest<'a> {
    fn into_existing(self) -> MieruSendExisting<'a> {
        MieruSendExisting {
            chain_tasks: self.chain_tasks,
            session_id: self.session.id,
            proxy: self.proxy,
            session: self.session,
            server: self.server,
            port: self.port,
            username: self.username,
            password: self.password,
            relay_chain: self.relay_chain,
            target: &self.session.target,
            target_port: self.session.port,
            payload: self.payload,
        }
    }
}
