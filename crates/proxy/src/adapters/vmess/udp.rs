use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::adapters::vmess::VmessAdapter;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::Proxy;

mod flow;
mod managed;

pub(super) fn vmess_udp_flow_config<'a>(
    id: &str,
    cipher: &'a str,
    stage: &'static str,
    upstream: Option<(&str, u16)>,
) -> Result<vmess::VmessUdpFlowConfig<'a>, FlowFailure> {
    vmess::VmessUdpFlowConfig::new(id, cipher).map_err(|error| FlowFailure {
        stage,
        error: zero_engine::EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid VMess UDP config: {error}"),
        )),
        upstream: upstream.map(|(server, port)| (server.to_string(), port)),
    })
}

impl VmessAdapter {
    pub(super) async fn start_udp_flow_impl(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        flow::start(self, dispatch, proxy, session, leaf, payload).await
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
        flow::start_relay_final_hop(self, dispatch, proxy, session, carrier, leaf, payload).await
    }
}
