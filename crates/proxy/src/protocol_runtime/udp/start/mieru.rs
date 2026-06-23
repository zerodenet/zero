use tokio::task::JoinSet;
use zero_core::Session;

use super::super::state::ProtocolUdpState;
use super::super::{
    ChainTask, FlowFailure, MieruRelayExisting, MieruSendExisting, MieruUdpRelayFlow,
};
use crate::runtime::Proxy;

impl ProtocolUdpState {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn start_mieru_udp_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        session: &Session,
        server: &str,
        port: u16,
        username: &str,
        password: &str,
        relay_chain: bool,
        payload: &[u8],
    ) -> Result<usize, FlowFailure> {
        self.mieru
            .send_existing(MieruSendExisting {
                chain_tasks,
                session_id: session.id,
                proxy,
                session,
                server,
                port,
                username,
                password,
                relay_chain,
                target: &session.target,
                target_port: session.port,
                payload,
            })
            .await
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
