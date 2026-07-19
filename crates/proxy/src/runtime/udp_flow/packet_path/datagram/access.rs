use zero_core::Address;

use super::model::{UdpDatagramDescriptor, UdpDatagramEndpoint, UdpDatagramKey, UdpDatagramSource};

#[cfg(feature = "udp-runtime")]
impl UdpDatagramDescriptor {
    pub(crate) fn key_part(&self) -> UdpDatagramKey {
        UdpDatagramKey {
            tag: self.tag.clone(),
            server: self.server.clone(),
            port: self.port,
            cache_key: self.cache_key.clone(),
        }
    }

    pub(crate) fn endpoint(&self) -> UdpDatagramEndpoint {
        UdpDatagramEndpoint {
            server: self.server.clone(),
            port: self.port,
        }
    }
}

#[cfg(feature = "udp-runtime")]
impl UdpDatagramSource {
    pub(crate) fn descriptor(&self) -> &UdpDatagramDescriptor {
        &self.descriptor
    }
}

#[cfg(feature = "udp-runtime")]
impl UdpDatagramEndpoint {
    pub(crate) fn target(&self) -> Address {
        Address::Domain(self.server.clone())
    }

    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    pub(crate) fn upstream(&self) -> (String, u16) {
        (self.server.clone(), self.port)
    }
}
