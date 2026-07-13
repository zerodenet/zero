use crate::runtime::udp_flow::registered::{
    boxed_registered_upstream_handler, UpstreamAssociationHandler, UpstreamAssociationStages,
    UpstreamAssociationTarget,
};

impl UpstreamAssociationTarget
    for zero_transport::socks5_transport::Socks5ManagedUdpAssociationTarget
{
    fn outbound_tag(&self) -> &str {
        self.outbound_tag()
    }

    fn log_parts(&self) -> (&str, &str, u16) {
        self.log_parts()
    }
}

pub(super) fn upstream_association_handler() -> Box<dyn UpstreamAssociationHandler> {
    boxed_registered_upstream_handler::<
        zero_transport::socks5_transport::Socks5ManagedUdpAssociationTarget,
        zero_transport::socks5_transport::Socks5UpstreamUdpAssociation,
    >(UpstreamAssociationStages::new(
        "udp_socks5_proxy",
        "udp_socks5_resume",
        "expected SOCKS5 UDP association target",
    ))
}
