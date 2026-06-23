use tokio::task::JoinSet;
use zero_core::Session;

use super::super::state::ProtocolUdpState;
use super::super::{ChainTask, FlowFailure, TrojanRelayExisting, TrojanSendExisting};
use crate::runtime::Proxy;

impl ProtocolUdpState {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn start_trojan_udp_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        session: &Session,
        server: &str,
        port: u16,
        password: &str,
        sni: Option<&str>,
        insecure: bool,
        client_fingerprint: Option<&str>,
        relay_chain: bool,
        payload: &[u8],
    ) -> Result<usize, FlowFailure> {
        self.trojan
            .send_existing(TrojanSendExisting {
                chain_tasks,
                session_id: session.id,
                proxy,
                session,
                server,
                port,
                password,
                sni,
                insecure,
                client_fingerprint,
                relay_chain,
                target: &session.target,
                target_port: session.port,
                payload,
            })
            .await
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn start_trojan_udp_relay_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        proxy: &Proxy,
        session: &Session,
        carrier: crate::transport::RelayCarrier,
        server: &str,
        port: u16,
        password: &str,
        sni: Option<&str>,
        insecure: bool,
        client_fingerprint: Option<&str>,
        payload: &[u8],
    ) -> Result<usize, FlowFailure> {
        self.trojan
            .send_relay_existing(TrojanRelayExisting {
                chain_tasks,
                session_id: session.id,
                stream: carrier.stream,
                tls_server_name: None,
                proxy,
                session,
                server,
                port,
                password,
                sni,
                insecure,
                client_fingerprint,
                target: &session.target,
                target_port: session.port,
                payload,
            })
            .await
    }
}
