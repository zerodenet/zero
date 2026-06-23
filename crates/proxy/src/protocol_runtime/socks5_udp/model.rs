/// SOCKS5 UDP association close reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum UpstreamAssociationCloseReason {
    Closed,
    IdleTimeout,
    Dropped,
}

pub(crate) struct Socks5UdpAssociationView<'a> {
    pub(crate) outbound_tag: &'a str,
}

pub(crate) struct ClosedSocks5UdpAssociation {
    pub(crate) outbound_tag: String,
    pub(crate) server: String,
    pub(crate) port: u16,
}

/// SOCKS5 UDP association context.
#[derive(Clone)]
pub(super) struct Socks5UdpAssociation {
    pub(super) tag: String,
    pub(super) server: String,
    pub(super) port: u16,
    pub(super) auth: Option<(String, String)>,
}
