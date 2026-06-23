use tokio::task::JoinSet;

use super::super::state::ProtocolUdpState;
use super::super::{ChainTask, FlowFailure};
use super::super::{VmessUdpFlow, VmessUdpRelayFlow};

impl ProtocolUdpState {
    pub(crate) async fn start_vmess_udp_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: VmessUdpFlow<'_>,
    ) -> Result<(), FlowFailure> {
        let transport = crate::protocol_runtime::vmess_udp::VmessUdpTransport {
            tls: flow.tls,
            ws: flow.ws,
            grpc: flow.grpc,
        };
        self.vmess
            .start_flow(
                chain_tasks,
                crate::protocol_runtime::vmess_udp::VmessUdpStartFlow {
                    proxy: flow.proxy,
                    session: flow.session,
                    server: flow.server,
                    port: flow.port,
                    id: flow.id,
                    cipher: flow.cipher,
                    mux_concurrency: flow.mux_concurrency,
                    transport,
                    payload: flow.payload,
                },
            )
            .await
            .map_err(|error| FlowFailure {
                stage: "udp_vmess_upstream",
                error,
                upstream: Some((flow.server.to_string(), flow.port)),
            })?;
        Ok(())
    }

    pub(crate) async fn start_vmess_udp_relay_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: VmessUdpRelayFlow<'_>,
    ) -> Result<(), FlowFailure> {
        let transport = crate::protocol_runtime::vmess_udp::VmessUdpTransport {
            tls: flow.tls,
            ws: flow.ws,
            grpc: flow.grpc,
        };
        self.vmess
            .start_relay_flow(
                chain_tasks,
                crate::protocol_runtime::vmess_udp::VmessUdpRelayFlow {
                    proxy: flow.proxy,
                    session: flow.session,
                    carrier: flow.carrier,
                    server: flow.server,
                    port: flow.port,
                    id: flow.id,
                    cipher: flow.cipher,
                    transport,
                    payload: flow.payload,
                },
            )
            .await
            .map_err(|error| FlowFailure {
                stage: "udp_vmess_relay_chain",
                error,
                upstream: None,
            })?;
        Ok(())
    }
}
