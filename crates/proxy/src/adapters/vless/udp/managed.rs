use crate::runtime::udp_flow::managed::{ManagedStreamFlowHandler, ManagedStreamFlowManager};

mod connector;

pub(super) fn handler() -> Box<dyn ManagedStreamFlowHandler> {
    Box::new(ManagedStreamFlowManager::<
        connector::VlessManagedUdpFlowResume,
    >::new(
        "vless_establish",
        "vless_relay_upstream",
        "vless_relay_establish",
        "vless_relay_send",
        "udp_vless_resume",
        "expected VLESS UDP flow resume",
    ))
}

pub(super) fn direct_resume(
    adapter: &crate::adapters::vless::VlessAdapter,
    protocol: vless::udp::VlessUdpFlowResume,
    transport: crate::transport::VlessUdpTransportOptions<'_>,
) -> connector::VlessManagedUdpFlowResume {
    connector::VlessManagedUdpFlowResume::direct(adapter.mux_pool.clone(), protocol, transport)
}

pub(super) fn relay_final_hop_resume(
    adapter: &crate::adapters::vless::VlessAdapter,
    protocol: vless::udp::VlessUdpFlowResume,
    transport: crate::transport::VlessUdpTransportOptions<'_>,
) -> connector::VlessManagedUdpFlowResume {
    connector::VlessManagedUdpFlowResume::relay_final_hop(
        adapter.mux_pool.clone(),
        protocol,
        transport,
    )
}

pub(super) fn relay_paired_transport_resume(
    adapter: &crate::adapters::vless::VlessAdapter,
    protocol: vless::udp::VlessUdpFlowResume,
    transport: crate::transport::VlessUdpTransportOptions<'_>,
) -> connector::VlessManagedUdpFlowResume {
    connector::VlessManagedUdpFlowResume::relay_paired_transport(
        adapter.mux_pool.clone(),
        protocol,
        transport,
    )
}
