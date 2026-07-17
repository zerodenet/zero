/// Runtime-neutral identity and endpoint facts exposed by a protocol-owned
/// outbound leaf.
pub trait ProtocolOutboundLeaf {
    fn tag(&self) -> &str;

    fn server(&self) -> &str;

    fn port(&self) -> u16;

    fn udp_relay_final_hop_error(&self) -> Option<&'static str> {
        None
    }
}
