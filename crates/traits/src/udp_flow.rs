pub trait ProtocolUdpFlowLeaf {
    type Resume: Send + Sync + core::fmt::Debug + 'static;

    fn direct_udp_resume(&self) -> Self::Resume;

    fn relay_final_hop_udp_resume(&self) -> Self::Resume;
}

pub trait ProtocolRelayTwoStreamUdpFlowLeaf: ProtocolUdpFlowLeaf {
    fn udp_relay_needs_two_streams(&self) -> bool;

    fn relay_two_stream_udp_resume(&self) -> Self::Resume;
}
