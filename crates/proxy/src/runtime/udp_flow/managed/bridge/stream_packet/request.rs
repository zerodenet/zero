use zero_core::Session;

use crate::runtime::Proxy;
use crate::transport::RelayCarrier;

pub(crate) struct ManagedStreamPacketRelay<'a> {
    pub(crate) carrier: RelayCarrier,
    pub(crate) tls_server_name: Option<&'a str>,
}

pub(crate) struct ManagedStreamPacketStartBridge<'a, T> {
    pub(super) proxy: Option<&'a Proxy>,
    pub(super) tag: &'a str,
    pub(super) session: &'a Session,
    pub(super) carrier: Option<RelayCarrier>,
    pub(super) tls_server_name: Option<&'a str>,
    pub(super) server: &'a str,
    pub(super) port: u16,
    pub(super) resume: T,
    pub(super) payload: &'a [u8],
    pub(super) relay_chain: bool,
}

impl<'a, T> ManagedStreamPacketStartBridge<'a, T> {
    pub(crate) fn direct(
        proxy: &'a Proxy,
        tag: &'a str,
        session: &'a Session,
        endpoint: (&'a str, u16),
        resume: T,
        payload: &'a [u8],
    ) -> Self {
        let (server, port) = endpoint;
        Self {
            proxy: Some(proxy),
            tag,
            session,
            carrier: None,
            tls_server_name: None,
            server,
            port,
            resume,
            payload,
            relay_chain: false,
        }
    }

    pub(crate) fn relay(
        proxy: Option<&'a Proxy>,
        tag: &'a str,
        session: &'a Session,
        relay: ManagedStreamPacketRelay<'a>,
        endpoint: (&'a str, u16),
        resume: T,
        payload: &'a [u8],
    ) -> Self {
        let (server, port) = endpoint;
        Self {
            proxy,
            tag,
            session,
            carrier: Some(relay.carrier),
            tls_server_name: relay.tls_server_name,
            server,
            port,
            resume,
            payload,
            relay_chain: true,
        }
    }
}
