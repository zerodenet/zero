use super::*;

impl MieruAdapter {
    pub(super) async fn start_udp_flow_impl(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let ResolvedLeafOutbound::Mieru {
            tag,
            server,
            port,
            username,
            password,
        } = leaf
        else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        let (protocol_state, chain_tasks) = dispatch.protocol_parts();
        let sent = protocol_state
            .start_mieru_udp_flow(crate::protocol_runtime::udp::MieruUdpFlowRequest {
                chain_tasks,
                proxy,
                session,
                server,
                port: *port,
                username,
                password,
                relay_chain: false,
                payload,
            })
            .await
            .map_err(|f: FlowFailure| FlowFailure {
                stage: f.stage,
                error: f.error,
                upstream: f.upstream,
            })?;
        Ok(FlowStartResult::Flow {
            outbound: Box::new(UdpFlowOutbound::Mieru {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                username: (*username).to_string(),
                password: (*password).to_string(),
                relay_chain: false,
            }),
            tx_bytes: sent as u64,
        })
    }

    pub(super) async fn start_udp_relay_final_hop_impl(
        &self,
        dispatch: &mut UdpDispatch,
        session: &Session,
        carrier: crate::transport::RelayCarrier,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        use crate::protocol_runtime::udp::MieruUdpRelayFlow;

        let ResolvedLeafOutbound::Mieru {
            tag,
            server,
            port,
            username,
            password,
        } = leaf
        else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        let (protocol_state, chain_tasks) = dispatch.protocol_parts();
        let sent = protocol_state
            .start_mieru_udp_relay_flow(
                chain_tasks,
                MieruUdpRelayFlow {
                    session,
                    carrier,
                    server,
                    port: *port,
                    username,
                    password,
                    payload,
                },
            )
            .await?;
        Ok(FlowStartResult::Flow {
            outbound: Box::new(UdpFlowOutbound::Mieru {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                username: (*username).to_string(),
                password: (*password).to_string(),
                relay_chain: true,
            }),
            tx_bytes: sent as u64,
        })
    }
}
