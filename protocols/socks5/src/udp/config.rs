use alloc::borrow::ToOwned;
use alloc::string::String;

use crate::outbound::{Socks5OutboundAuth, Socks5UdpFlowResume};

use super::association::Socks5UdpAssociationTarget;

fn udp_cache_key(tag: &str, server: &str, port: u16, username: Option<&str>) -> String {
    let auth = username
        .map(|value| alloc::format!("|auth:{value}"))
        .unwrap_or_default();
    alloc::format!("socks5|{tag}|{server}:{port}{auth}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Socks5UdpFlowConfig<'a> {
    tag: &'a str,
    server: &'a str,
    port: u16,
    username: Option<&'a str>,
    password: Option<&'a str>,
}

impl<'a> Socks5UdpFlowConfig<'a> {
    pub fn new(
        tag: &'a str,
        server: &'a str,
        port: u16,
        username: Option<&'a str>,
        password: Option<&'a str>,
    ) -> Self {
        Self {
            tag,
            server,
            port,
            username,
            password,
        }
    }

    pub fn flow_resume(&self) -> Socks5UdpFlowResume {
        Socks5UdpFlowResume::new(self.auth())
    }

    pub fn auth(&self) -> Option<Socks5OutboundAuth<'a>> {
        self.username
            .zip(self.password)
            .map(|(username, password)| Socks5OutboundAuth { username, password })
    }

    pub fn cache_key(&self) -> String {
        udp_cache_key(self.tag, self.server, self.port, self.username)
    }

    pub fn association_target(&self) -> Socks5UdpAssociationTarget {
        self.flow_resume().association_target(
            self.tag.to_owned(),
            self.server.to_owned(),
            self.port,
        )
    }

    pub fn packet_path_spec(&self) -> Socks5UdpPacketPathSpec {
        Socks5UdpPacketPathSpec {
            cache_key: self.cache_key(),
            association_target: self.association_target(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5UdpPacketPathSpec {
    cache_key: String,
    association_target: Socks5UdpAssociationTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5UdpPacketPathCarrierBuild {
    cache_key: String,
    server: String,
    port: u16,
    association_target: Socks5UdpAssociationTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5UdpPacketPathCarrierDescriptor {
    cache_key: String,
    server: String,
    port: u16,
}

impl Socks5UdpPacketPathSpec {
    pub fn carrier_build(&self) -> Socks5UdpPacketPathCarrierBuild {
        Socks5UdpPacketPathCarrierBuild {
            cache_key: self.cache_key.clone(),
            server: self.association_target.server().to_owned(),
            port: self.association_target.port(),
            association_target: self.association_target.clone(),
        }
    }

    pub fn carrier_descriptor(&self) -> Socks5UdpPacketPathCarrierDescriptor {
        Socks5UdpPacketPathCarrierDescriptor {
            cache_key: self.cache_key.clone(),
            server: self.association_target.server().to_owned(),
            port: self.association_target.port(),
        }
    }
}

impl Socks5UdpPacketPathCarrierBuild {
    pub fn into_association_target(self) -> Socks5UdpAssociationTarget {
        self.association_target
    }
}

pub fn packet_path_carrier_association_target(
    carrier: Socks5UdpPacketPathCarrierBuild,
) -> Socks5UdpAssociationTarget {
    carrier.into_association_target()
}

impl Socks5UdpPacketPathCarrierDescriptor {
    pub fn into_parts(self) -> (String, String, u16) {
        (self.cache_key, self.server, self.port)
    }
}
