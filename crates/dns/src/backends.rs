//! Internal enum over all DNS resolver backends.

use std::io;

use zero_config::DnsServerConfig;
use zero_traits::{DnsResolver, IpAddress};

use crate::system::TokioSystemResolver;
#[cfg(feature = "udp")]
use crate::udp::UdpDnsResolver;

/// All DNS resolver backend variants.
pub(crate) enum ResolverBackend {
    System(TokioSystemResolver),
    #[cfg(feature = "udp")]
    Udp(UdpDnsResolver),
    // DoH, DoT in v2
}

impl ResolverBackend {
    /// Build a backend from its config.
    pub(crate) fn build(server: &DnsServerConfig) -> io::Result<Self> {
        match server {
            DnsServerConfig::System => Ok(Self::System(TokioSystemResolver)),
            #[cfg(feature = "udp")]
            DnsServerConfig::Udp { address, port } => {
                let addr = format!("{address}:{port}");
                Ok(Self::Udp(UdpDnsResolver::new(&addr)))
            }
            #[cfg(not(feature = "udp"))]
            DnsServerConfig::Udp { .. } => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "UDP DNS backend is not compiled (enable feature `dns-over-udp`)",
            )),
            DnsServerConfig::Doh { .. } => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "DNS-over-HTTPS is not yet implemented",
            )),
            DnsServerConfig::Dot { .. } => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "DNS-over-TLS is not yet implemented",
            )),
        }
    }

    pub(crate) async fn resolve(&self, domain: &str) -> io::Result<Vec<IpAddress>> {
        match self {
            Self::System(r) => r.resolve(domain).await,
            #[cfg(feature = "udp")]
            Self::Udp(r) => r.resolve(domain).await,
        }
    }
}
