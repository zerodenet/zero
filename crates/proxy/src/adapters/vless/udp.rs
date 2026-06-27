use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::adapters::vless::VlessAdapter;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::Proxy;

mod flow;
mod managed;

pub(super) fn vless_udp_flow_config<'a>(
    id: &str,
    flow: Option<&'a str>,
    stage: &'static str,
    upstream: Option<(&str, u16)>,
) -> Result<vless::VlessUdpFlowConfig<'a>, FlowFailure> {
    vless::VlessUdpFlowConfig::new(id, flow).map_err(|error| FlowFailure {
        stage,
        error: zero_engine::EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid VLESS UDP config: {error}"),
        )),
        upstream: upstream.map(|(server, port)| (server.to_string(), port)),
    })
}

impl VlessAdapter {
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

    pub(super) fn udp_relay_needs_two_streams_impl(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool {
        let _ = self;
        matches!(
            leaf,
            ResolvedLeafOutbound::Vless {
                split_http: Some(cfg),
                ..
            } if !zero_transport::split_http::XhttpMode::parse(&cfg.mode).is_single_connection()
        )
    }

    pub(super) async fn start_udp_relay_two_stream_impl(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        chain: Vec<ResolvedLeafOutbound<'_>>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        flow::start_relay_two_stream(self, dispatch, proxy, session, chain, payload).await
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
