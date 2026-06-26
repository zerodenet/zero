use tokio::task::JoinSet;

use super::super::ProtocolUdpState;
use crate::protocol_runtime::udp::ss_manager::model::SsSendExisting;
use crate::protocol_runtime::udp::{ChainTask, FlowFailure, ProtocolUdpFlowSnapshot};
use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;
use crate::runtime::Proxy;

pub(super) struct ExistingFlow<'a> {
    pub(super) server: &'a str,
    pub(super) port: u16,
    pub(super) password: &'a str,
    pub(super) datagram_cache_key: &'a str,
    pub(super) cipher_kind: shadowsocks::CipherKind,
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
        .shadowsocks
        .send_existing(SsSendExisting {
            chain_tasks,
            session_id: flow.session.id,
            proxy,
            server: existing.server,
            port: existing.port,
            cache_key: existing.datagram_cache_key.to_owned(),
            codec: std::sync::Arc::new(shadowsocks::udp_flow_codec(
                existing.cipher_kind,
                existing.password.as_bytes(),
            )),
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
    let ProtocolUdpFlowSnapshot::Shadowsocks {
        password,
        datagram_cache_key,
        cipher_kind,
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
                datagram_cache_key,
                cipher_kind: *cipher_kind,
                payload,
            },
        )
        .await,
    )
}
