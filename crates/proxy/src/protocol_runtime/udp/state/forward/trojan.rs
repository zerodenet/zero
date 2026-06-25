use tokio::task::JoinSet;

use super::super::ProtocolUdpState;
use crate::protocol_runtime::udp::trojan_manager::model::TrojanSendExisting;
use crate::protocol_runtime::udp::{ChainTask, FlowFailure, ProtocolUdpFlowSnapshot};
use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;
use crate::runtime::Proxy;

pub(super) struct ExistingFlow<'a> {
    pub(super) server: &'a str,
    pub(super) port: u16,
    pub(super) password: &'a str,
    pub(super) sni: Option<&'a str>,
    pub(super) insecure: bool,
    pub(super) client_fingerprint: Option<&'a str>,
    pub(super) relay_chain: bool,
    pub(super) payload: &'a [u8],
}

pub(super) async fn forward(
    state: &mut ProtocolUdpState,
    chain_tasks: &mut JoinSet<ChainTask>,
    proxy: &Proxy,
    flow: &UdpFlowSnapshot,
    existing: ExistingFlow<'_>,
) -> Result<usize, FlowFailure> {
    state
        .trojan
        .send_existing(TrojanSendExisting {
            chain_tasks,
            session_id: flow.session.id,
            proxy,
            session: &flow.session,
            server: existing.server,
            port: existing.port,
            password: existing.password,
            sni: existing.sni,
            insecure: existing.insecure,
            client_fingerprint: existing.client_fingerprint,
            relay_chain: existing.relay_chain,
            target: &flow.session.target,
            target_port: flow.session.port,
            payload: existing.payload,
        })
        .await
}

pub(super) async fn forward_if_matches(
    state: &mut ProtocolUdpState,
    chain_tasks: &mut JoinSet<ChainTask>,
    proxy: &Proxy,
    flow: &UdpFlowSnapshot,
    snapshot: &ProtocolUdpFlowSnapshot,
    payload: &[u8],
) -> Option<Result<usize, FlowFailure>> {
    let ProtocolUdpFlowSnapshot::Trojan {
        password,
        sni,
        insecure,
        client_fingerprint,
        relay_chain,
    } = snapshot
    else {
        return None;
    };

    let upstream = flow
        .outbound
        .upstream()
        .expect("protocol flow should have upstream");

    Some(
        forward(
            state,
            chain_tasks,
            proxy,
            flow,
            ExistingFlow {
                server: upstream.server,
                port: upstream.port,
                password,
                sni: sni.as_deref(),
                insecure: *insecure,
                client_fingerprint: client_fingerprint.as_deref(),
                relay_chain: *relay_chain,
                payload,
            },
        )
        .await,
    )
}
