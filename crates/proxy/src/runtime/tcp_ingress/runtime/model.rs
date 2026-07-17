use std::net::SocketAddr;
use std::time::Duration;

use crate::protocol_registry::TcpRuntimeServices;

#[derive(Clone)]
pub(crate) struct TcpIngressRuntime {
    pub(super) services: TcpRuntimeServices,
    pub(super) inbound_tag: String,
    pub(super) source_addr: Option<SocketAddr>,
}

impl TcpIngressRuntime {
    pub(crate) fn new(
        services: TcpRuntimeServices,
        inbound_tag: String,
        source_addr: Option<SocketAddr>,
    ) -> Self {
        Self {
            services,
            inbound_tag,
            source_addr,
        }
    }

    pub(crate) fn inbound_tag(&self) -> &str {
        &self.inbound_tag
    }

    pub(crate) fn source_addr(&self) -> Option<SocketAddr> {
        self.source_addr
    }

    #[cfg(feature = "managed-stream-runtime")]
    pub(crate) fn without_source_addr(&self) -> Self {
        Self {
            services: self.services.clone(),
            inbound_tag: self.inbound_tag.clone(),
            source_addr: None,
        }
    }

    pub(crate) fn runtime_services(&self) -> TcpRuntimeServices {
        self.services.clone()
    }

    pub(crate) fn idle_timeout(&self) -> Duration {
        Duration::from_secs(
            self.services
                .config()
                .inbounds
                .iter()
                .find(|i| i.tag == self.inbound_tag)
                .and_then(|i| i.idle_timeout_secs)
                .unwrap_or(300),
        )
    }
}
