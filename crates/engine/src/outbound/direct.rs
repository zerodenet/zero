use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use zero_core::{Address, Error, Session};
use zero_platform_tokio::{TokioResolver, TokioSocket};
use zero_traits::{DnsResolver, IpAddress};

#[derive(Debug, Default, Clone, Copy)]
pub struct DirectOutbound;

impl DirectOutbound {
    pub fn validate(&self, session: &Session) -> Result<(), Error> {
        if session.port == 0 {
            return Err(Error::Config("target port is required"));
        }

        Ok(())
    }

    pub async fn connect(
        &self,
        session: &Session,
        resolver: &TokioResolver,
    ) -> Result<TokioSocket, Error> {
        self.validate(session)?;

        let addr = match &session.target {
            Address::Domain(domain) => {
                resolve_host(
                    domain,
                    session.port,
                    resolver,
                    "failed to resolve direct target",
                )
                .await?
            }
            Address::Ipv4(bytes) => {
                SocketAddr::new(IpAddr::V4(Ipv4Addr::from(*bytes)), session.port)
            }
            Address::Ipv6(bytes) => {
                SocketAddr::new(IpAddr::V6(Ipv6Addr::from(*bytes)), session.port)
            }
        };

        TokioSocket::connect_addr(addr)
            .await
            .map_err(|_| Error::Io("failed to connect direct target"))
    }

    pub async fn connect_host(
        &self,
        host: &str,
        port: u16,
        resolver: &TokioResolver,
    ) -> Result<TokioSocket, Error> {
        if port == 0 {
            return Err(Error::Config("target port is required"));
        }

        let addr = resolve_host(host, port, resolver, "failed to resolve upstream target").await?;

        TokioSocket::connect_addr(addr)
            .await
            .map_err(|_| Error::Io("failed to connect upstream target"))
    }
}

fn socket_addr_from_ip(ip: IpAddress, port: u16) -> SocketAddr {
    match ip {
        IpAddress::V4(bytes) => SocketAddr::new(IpAddr::V4(Ipv4Addr::from(bytes)), port),
        IpAddress::V6(bytes) => SocketAddr::new(IpAddr::V6(Ipv6Addr::from(bytes)), port),
    }
}

async fn resolve_host(
    host: &str,
    port: u16,
    resolver: &TokioResolver,
    error_message: &'static str,
) -> Result<SocketAddr, Error> {
    let resolved = resolver
        .resolve(host)
        .await
        .map_err(|_| Error::Io(error_message))?;
    let ip = resolved
        .into_iter()
        .next()
        .ok_or(Error::Io("target resolved to no addresses"))?;

    Ok(socket_addr_from_ip(ip, port))
}
