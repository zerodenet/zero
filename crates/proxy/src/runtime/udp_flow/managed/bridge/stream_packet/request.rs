use zero_core::Session;

use crate::runtime::Proxy;
use crate::transport::RelayCarrier;

pub(super) struct ManagedStreamPacketStartBridge<'a, T> {
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
    pub(super) fn direct(
        proxy: &'a Proxy,
        tag: &'a str,
        session: &'a Session,
        server: &'a str,
        port: u16,
        resume: T,
        payload: &'a [u8],
    ) -> Self {
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

    #[allow(clippy::too_many_arguments)]
    pub(super) fn relay(
        proxy: Option<&'a Proxy>,
        tag: &'a str,
        session: &'a Session,
        carrier: RelayCarrier,
        tls_server_name: Option<&'a str>,
        server: &'a str,
        port: u16,
        resume: T,
        payload: &'a [u8],
    ) -> Self {
        Self {
            proxy,
            tag,
            session,
            carrier: Some(carrier),
            tls_server_name,
            server,
            port,
            resume,
            payload,
            relay_chain: true,
        }
    }
}
