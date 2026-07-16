use std::path::Path;

use crate::protocol_registry::{TcpRuntimeServices, UdpRuntimeServices};

#[derive(Clone)]
pub(crate) struct UdpIngressRuntime {
    pub(super) tcp_services: TcpRuntimeServices,
    pub(super) services: UdpRuntimeServices,
}

impl UdpIngressRuntime {
    pub(crate) fn new(tcp_services: TcpRuntimeServices) -> Self {
        let services = UdpRuntimeServices::new(tcp_services.clone());
        Self {
            tcp_services,
            services,
        }
    }

    pub(crate) fn services(&self) -> &UdpRuntimeServices {
        &self.services
    }

    pub(crate) fn runtime_services(&self) -> UdpRuntimeServices {
        self.services.clone()
    }

    pub(crate) fn source_dir(&self) -> Option<&Path> {
        self.tcp_services.config().source_dir()
    }
}
