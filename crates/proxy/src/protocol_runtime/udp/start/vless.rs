use tokio::task::JoinSet;

use super::super::state::ProtocolUdpState;
use super::super::FlowFailure;
use crate::protocol_runtime::vless_udp::model::{
    VlessUdpFlow, VlessUdpRelayFinalHop, VlessUdpRelayTwoStream,
};
use crate::runtime::udp_flow::packet_path::ChainTask;

impl ProtocolUdpState {
    pub(crate) async fn start_vless_udp_flow(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: VlessUdpFlow<'_>,
    ) -> Result<(), FlowFailure> {
        let transport = crate::transport::VlessUdpTransportOptions {
            tls: flow.tls,
            reality: flow.reality,
            ws: flow.ws,
            grpc: flow.grpc,
            h2: flow.h2,
            http_upgrade: flow.http_upgrade,
            split_http: flow.split_http,
            quic: flow.quic,
            source_dir: flow.proxy.config.source_dir(),
        };
        self.start_vless_cached_flow(
            chain_tasks,
            crate::protocol_runtime::vless_udp::model::VlessUdpStartFlow {
                proxy: flow.proxy,
                session: flow.session,
                server: flow.server,
                port: flow.port,
                identity: flow.identity,
                flow: flow.flow,
                transport,
                payload: flow.payload,
            },
        )
        .await
        .map_err(|error| FlowFailure {
            stage: "udp_vless_upstream",
            error,
            upstream: Some((flow.server.to_string(), flow.port)),
        })?;
        Ok(())
    }

    pub(crate) async fn start_vless_udp_relay_two_stream(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: VlessUdpRelayTwoStream<'_>,
    ) -> Result<(), FlowFailure> {
        self.start_vless_cached_relay_two_stream(
            chain_tasks,
            crate::protocol_runtime::vless_udp::model::VlessUdpRelayTwoStream {
                proxy: flow.proxy,
                session: flow.session,
                post_carrier: flow.post_carrier,
                get_carrier: flow.get_carrier,
                identity: flow.identity,
                split_http: flow.split_http,
                payload: flow.payload,
            },
        )
        .await
        .map_err(|error| FlowFailure {
            stage: "udp_vless_relay_chain",
            error,
            upstream: None,
        })?;
        Ok(())
    }

    pub(crate) async fn start_vless_udp_relay_final_hop(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        flow: VlessUdpRelayFinalHop<'_>,
    ) -> Result<(), FlowFailure> {
        let transport = crate::transport::VlessUdpTransportOptions {
            tls: flow.tls,
            reality: flow.reality,
            ws: flow.ws,
            grpc: flow.grpc,
            h2: flow.h2,
            http_upgrade: flow.http_upgrade,
            split_http: flow.split_http,
            quic: None,
            source_dir: flow.proxy.config.source_dir(),
        };
        self.start_vless_cached_relay_final_hop(
            chain_tasks,
            crate::protocol_runtime::vless_udp::model::VlessUdpRelayFinalHopStart {
                proxy: flow.proxy,
                session: flow.session,
                carrier: flow.carrier,
                identity: flow.identity,
                transport,
                payload: flow.payload,
            },
        )
        .await
        .map_err(|error| FlowFailure {
            stage: "udp_vless_relay_chain",
            error,
            upstream: None,
        })?;
        Ok(())
    }
}
