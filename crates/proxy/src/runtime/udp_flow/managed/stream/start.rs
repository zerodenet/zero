use super::super::model::ManagedRelayExistingSend;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
use super::super::model::ManagedStreamExistingSend;
use super::super::state::flow_mismatch;
use super::model::ManagedStreamState;
use crate::runtime::udp_flow::managed::flow::ManagedRelayStreamFlow;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
use crate::runtime::udp_flow::managed::flow::ManagedStreamPacketFlow;
use crate::runtime::udp_flow::result::FlowFailure;

impl ManagedStreamState {
    #[cfg(any(
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    pub(in crate::runtime::udp_flow::managed) async fn start_stream_packet_flow(
        &mut self,
        request: ManagedStreamPacketFlow<'_>,
    ) -> Result<usize, FlowFailure> {
        for handler in &mut self.stream_packet_handlers {
            if !handler.supports_managed_existing(&request.resume) {
                continue;
            }
            return handler
                .send_managed_existing(ManagedStreamExistingSend::stream_packet(request))
                .await;
        }
        Err(flow_mismatch(
            "udp_stream_packet_resume",
            request.server,
            request.port,
            "expected stream-packet UDP flow resume",
        ))
    }

    pub(in crate::runtime::udp_flow::managed) async fn start_relay_stream_flow(
        &mut self,
        request: ManagedRelayStreamFlow<'_>,
    ) -> Result<usize, FlowFailure> {
        for handler in &mut self.relay_handlers {
            if !handler.supports_managed_relay_existing(&request.resume) {
                continue;
            }
            return handler
                .send_managed_relay_existing(ManagedRelayExistingSend::relay_stream(request))
                .await;
        }
        Err(flow_mismatch(
            "udp_relay_stream_resume",
            request.server,
            request.port,
            "expected relay-stream UDP flow resume",
        ))
    }
}
