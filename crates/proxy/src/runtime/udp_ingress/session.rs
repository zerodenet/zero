use zero_core::Session;
use zero_engine::SessionHandle;
use zero_traits::DnsResolver;

use super::model::UdpIngressRuntime;
use crate::runtime::tcp_ingress::apply_kernel_rate_limits_from_config;

impl UdpIngressRuntime {
    pub(crate) async fn resolve_local_dns(&self, domain: &str) {
        let _ = self.tcp_services.resolver().resolve(domain).await;
    }

    pub(crate) fn prepare_udp_session(&self, session: &mut Session, inbound_tag: &str) {
        self.tcp_services
            .engine()
            .prepare_session(session, inbound_tag);
        apply_kernel_rate_limits_from_config(self.tcp_services.config(), session, inbound_tag);
    }

    pub(crate) fn track_session(&self, session_id: u64) -> SessionHandle {
        self.tcp_services.engine().track_session(session_id)
    }

    pub(crate) async fn resolve_fake_ip_target(&self, session: &mut Session) {
        use zero_core::Address;
        use zero_traits::IpAddress;

        let ip = match &session.target {
            Address::Ipv4(octets) => IpAddress::V4(*octets),
            Address::Ipv6(octets) => IpAddress::V6(*octets),
            _ => return,
        };
        if let Some(domain) = self.tcp_services.resolver().lookup_fake_ip(&ip).await {
            session.target = Address::Domain(domain);
        }
    }

    #[cfg(any(
        feature = "upstream-association-runtime",
        feature = "managed-stream-runtime"
    ))]
    pub(crate) fn record_session_inbound_traffic(
        &self,
        session_id: u64,
        traffic: crate::transport::StreamTraffic,
    ) {
        self.services
            .record_session_inbound_traffic(session_id, traffic);
    }
}
