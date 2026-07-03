use crate::runtime::udp_flow::managed::{ManagedStreamFlowHandler, ManagedStreamFlowManager};

mod connector;

pub(super) fn handler() -> Box<dyn ManagedStreamFlowHandler> {
    Box::new(ManagedStreamFlowManager::<
        connector::VmessManagedUdpFlowResume,
    >::new(
        "vmess_establish",
        "vmess_relay_upstream",
        "vmess_relay_establish",
        "vmess_relay_send",
        "udp_vmess_resume",
        "expected VMess UDP flow resume",
    ))
}

pub(super) fn resume(
    adapter: &crate::adapters::vmess::VmessAdapter,
    protocol: vmess::udp::VmessUdpFlowResume,
    mux_concurrency: Option<u32>,
    transport: crate::transport::VmessTransportOptions<'_>,
) -> connector::VmessManagedUdpFlowResume {
    connector::VmessManagedUdpFlowResume::new(
        adapter.mux_pool.clone(),
        protocol,
        mux_concurrency,
        transport,
    )
}
