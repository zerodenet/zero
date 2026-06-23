use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::adapters::common::unreachable_udp_leaf;
use crate::adapters::vmess::VmessAdapter;
use crate::protocol_adapter::ProtocolAdapter;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::Proxy;

impl VmessAdapter {
    pub(super) async fn start_udp_flow_impl(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        use crate::protocol_runtime::udp::VmessUdpFlow;

        let ResolvedLeafOutbound::Vmess {
            tag,
            server,
            port,
            id,
            cipher,
            mux_concurrency,
            mux_idle_timeout_secs: _,
            tls,
            ws,
            grpc,
        } = leaf
        else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        let tag_owned = (*tag).to_string();
        let (protocol_state, chain_tasks) = dispatch.protocol_parts();
        protocol_state
            .start_vmess_udp_flow(
                chain_tasks,
                VmessUdpFlow {
                    proxy,
                    session,
                    server,
                    port: *port,
                    id,
                    cipher,
                    mux_concurrency: *mux_concurrency,
                    tls: *tls,
                    ws: *ws,
                    grpc: *grpc,
                    payload,
                },
            )
            .await?;

        Ok(FlowStartResult::VmessFlow {
            session_id: session.id,
            tag: tag_owned,
        })
    }

    pub(super) async fn start_udp_relay_final_hop_impl(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        carrier: crate::transport::RelayCarrier,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        use crate::protocol_runtime::udp::VmessUdpRelayFlow;

        let ResolvedLeafOutbound::Vmess {
            tag,
            server,
            port,
            id,
            cipher,
            tls,
            ws,
            grpc,
            ..
        } = leaf
        else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        let tag_owned = (*tag).to_string();
        let (protocol_state, chain_tasks) = dispatch.protocol_parts();
        protocol_state
            .start_vmess_udp_relay_flow(
                chain_tasks,
                VmessUdpRelayFlow {
                    proxy,
                    session,
                    carrier,
                    server,
                    port: *port,
                    id,
                    cipher,
                    tls: *tls,
                    ws: *ws,
                    grpc: *grpc,
                    payload,
                },
            )
            .await?;

        Ok(FlowStartResult::VmessFlow {
            session_id: session.id,
            tag: tag_owned,
        })
    }
}
