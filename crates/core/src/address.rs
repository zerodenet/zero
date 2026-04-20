use alloc::string::String;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Address {
    Domain(String),
    Ipv4([u8; 4]),
    Ipv6([u8; 16]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AddressFamily {
    Domain,
    Ipv4,
    Ipv6,
}

impl Address {
    pub fn family(&self) -> AddressFamily {
        match self {
            Self::Domain(_) => AddressFamily::Domain,
            Self::Ipv4(_) => AddressFamily::Ipv4,
            Self::Ipv6(_) => AddressFamily::Ipv6,
        }
    }
}
