use tokio::task::JoinSet;
use zero_core::Session;

use super::super::state::ProtocolUdpState;
use super::super::trojan_manager::model::{TrojanRelayExisting, TrojanSendExisting};
use super::super::{ChainTask, FlowFailure};
use crate::runtime::Proxy;

impl ProtocolUdpState {
    pub(crate) async fn start_trojan_udp_flow(
        &mut self,
        request: TrojanUdpFlowRequest<'_>,
    ) -> Result<usize, FlowFailure> {
        self.trojan.send_existing(request.into_existing()).await
    }

    pub(crate) async fn start_trojan_udp_relay_flow(
        &mut self,
        request: TrojanUdpRelayFlowRequest<'_>,
    ) -> Result<usize, FlowFailure> {
        self.trojan
            .send_relay_existing(request.into_existing())
            .await
    }
}

pub(crate) struct TrojanUdpFlowRequest<'a> {
    pub chain_tasks: &'a mut JoinSet<ChainTask>,
    pub proxy: &'a Proxy,
    pub session: &'a Session,
    pub server: &'a str,
    pub port: u16,
    pub password: &'a str,
    pub sni: Option<&'a str>,
    pub insecure: bool,
    pub client_fingerprint: Option<&'a str>,
    pub relay_chain: bool,
    pub payload: &'a [u8],
}

impl<'a> TrojanUdpFlowRequest<'a> {
    fn into_existing(self) -> TrojanSendExisting<'a> {
        TrojanSendExisting {
            chain_tasks: self.chain_tasks,
            session_id: self.session.id,
            proxy: self.proxy,
            session: self.session,
            server: self.server,
            port: self.port,
            password: self.password,
            sni: self.sni,
            insecure: self.insecure,
            client_fingerprint: self.client_fingerprint,
            relay_chain: self.relay_chain,
            target: &self.session.target,
            target_port: self.session.port,
            payload: self.payload,
        }
    }
}

pub(crate) struct TrojanUdpRelayFlowRequest<'a> {
    pub chain_tasks: &'a mut JoinSet<ChainTask>,
    pub proxy: &'a Proxy,
    pub session: &'a Session,
    pub carrier: crate::transport::RelayCarrier,
    pub server: &'a str,
    pub port: u16,
    pub password: &'a str,
    pub sni: Option<&'a str>,
    pub insecure: bool,
    pub client_fingerprint: Option<&'a str>,
    pub payload: &'a [u8],
}

impl<'a> TrojanUdpRelayFlowRequest<'a> {
    fn into_existing(self) -> TrojanRelayExisting<'a> {
        TrojanRelayExisting {
            chain_tasks: self.chain_tasks,
            session_id: self.session.id,
            stream: self.carrier.stream,
            tls_server_name: None,
            proxy: self.proxy,
            session: self.session,
            server: self.server,
            port: self.port,
            password: self.password,
            sni: self.sni,
            insecure: self.insecure,
            client_fingerprint: self.client_fingerprint,
            target: &self.session.target,
            target_port: self.session.port,
            payload: self.payload,
        }
    }
}
