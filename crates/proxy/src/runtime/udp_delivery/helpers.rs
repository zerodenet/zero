//! Response accounting, delivery models, and lifecycle helpers.

use std::net::SocketAddr;

#[cfg(feature = "socks5")]
use tokio::time::{sleep_until, Instant as TokioInstant};
use zero_core::Address;

use crate::logging::log_session_finished;
use crate::protocol_registry::UdpRuntimeServices;
use crate::runtime::udp_dispatch::UdpDispatch;

#[cfg(feature = "socks5")]
use crate::runtime::udp_flow::response::UpstreamUdpResponse;
use crate::runtime::udp_flow::sessions::CompletedUdpFlow;

pub(crate) fn log_completed_udp_flow(completed: CompletedUdpFlow) {
    log_session_finished(
        &completed.record,
        completed
            .upstream
            .as_ref()
            .map(|(server, port)| (server.as_str(), *port)),
    );
}

#[cfg(feature = "socks5")]
pub(crate) async fn wait_for_upstream_idle(deadline: Option<TokioInstant>) {
    match deadline {
        Some(deadline) => sleep_until(deadline).await,
        None => std::future::pending::<()>().await,
    }
}

pub(crate) fn address_from_socket_addr(addr: SocketAddr) -> Address {
    match zero_platform_tokio::socket_addr_to_ip(addr) {
        zero_traits::IpAddress::V4(ip) => Address::Ipv4(ip),
        zero_traits::IpAddress::V6(ip) => Address::Ipv6(ip),
    }
}

pub(crate) fn udp_response_target_from_socket_addr(addr: SocketAddr) -> (Address, u16) {
    (address_from_socket_addr(addr), addr.port())
}

pub(crate) fn record_udp_inbound_response_rx(
    services: &UdpRuntimeServices,
    session_id: Option<u64>,
    payload_len: usize,
) {
    if let Some(session_id) = session_id {
        services.record_session_outbound_rx(session_id, payload_len as u64);
    }
}

pub(crate) fn record_udp_inbound_response_tx(
    services: &UdpRuntimeServices,
    session_id: Option<u64>,
    written_len: usize,
) {
    if let Some(session_id) = session_id {
        services.record_session_inbound_tx(session_id, written_len as u64);
    }
}

pub(crate) struct UdpInboundResponseAccounting {
    services: UdpRuntimeServices,
    session_id: Option<u64>,
}

impl UdpInboundResponseAccounting {
    pub(crate) fn record_received(
        services: &UdpRuntimeServices,
        session_id: Option<u64>,
        payload_len: usize,
    ) -> Self {
        record_udp_inbound_response_rx(services, session_id, payload_len);
        Self {
            services: services.clone(),
            session_id,
        }
    }

    pub(crate) fn record_sent(&self, written_len: usize) {
        record_udp_inbound_response_tx(&self.services, self.session_id, written_len);
    }

    #[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
    pub(crate) fn session_id(&self) -> Option<u64> {
        self.session_id
    }
}

#[cfg(feature = "socks5")]
pub(crate) struct UdpUpstreamResponseParts {
    pub(crate) target: Address,
    pub(crate) port: u16,
    pub(crate) payload: Vec<u8>,
    pub(crate) accounting: UdpInboundResponseAccounting,
}

pub(crate) struct UdpDirectResponseParts<'payload> {
    pub(crate) target: Address,
    pub(crate) port: u16,
    pub(crate) payload: &'payload [u8],
    pub(crate) accounting: UdpInboundResponseAccounting,
}

pub(crate) struct UdpChainResponseParts {
    pub(crate) target: Address,
    pub(crate) port: u16,
    pub(crate) payload: Vec<u8>,
    pub(crate) accounting: UdpInboundResponseAccounting,
}

#[cfg(feature = "socks5")]
pub(crate) fn record_upstream_udp_response_received(
    services: &UdpRuntimeServices,
    dispatch: &mut UdpDispatch,
    timeout: std::time::Duration,
    response: UpstreamUdpResponse,
) -> UdpUpstreamResponseParts {
    services.record_udp_upstream_packet_received();
    dispatch.touch_upstream_idle(timeout);
    let (target, port, payload) = response.into_parts();
    let session_id = match dispatch.upstream_association_view() {
        Some(association) => {
            dispatch.upstream_response_session_id(association.outbound_tag, &target, port)
        }
        None => udp_response_session_id(dispatch, &target, port),
    };
    let accounting =
        UdpInboundResponseAccounting::record_received(services, session_id, payload.len());
    UdpUpstreamResponseParts {
        target,
        port,
        payload,
        accounting,
    }
}

pub(crate) fn record_direct_udp_response_received(
    services: &UdpRuntimeServices,
    dispatch: &UdpDispatch,
    sender: SocketAddr,
    payload_len: usize,
) -> UdpInboundResponseAccounting {
    let session_id = dispatch.direct_response_session_id(sender);
    UdpInboundResponseAccounting::record_received(services, session_id, payload_len)
}

pub(crate) fn record_direct_udp_response_parts<'payload>(
    services: &UdpRuntimeServices,
    dispatch: &UdpDispatch,
    sender: SocketAddr,
    payload: &'payload [u8],
) -> UdpDirectResponseParts<'payload> {
    let accounting = record_direct_udp_response_received(services, dispatch, sender, payload.len());
    let (target, port) = udp_response_target_from_socket_addr(sender);
    UdpDirectResponseParts {
        target,
        port,
        payload,
        accounting,
    }
}

pub(crate) fn record_chain_udp_response_received(
    services: &UdpRuntimeServices,
    session_id: Option<u64>,
    payload_len: usize,
) -> UdpInboundResponseAccounting {
    UdpInboundResponseAccounting::record_received(services, session_id, payload_len)
}

pub(crate) fn record_chain_udp_response_parts(
    services: &UdpRuntimeServices,
    target: Address,
    port: u16,
    payload: Vec<u8>,
    session_id: Option<u64>,
) -> UdpChainResponseParts {
    let accounting = record_chain_udp_response_received(services, session_id, payload.len());
    UdpChainResponseParts {
        target,
        port,
        payload,
        accounting,
    }
}

#[cfg(feature = "socks5")]
pub(crate) fn udp_response_session_id(
    dispatch: &UdpDispatch,
    target: &Address,
    port: u16,
) -> Option<u64> {
    dispatch.session_id_by_target(target, port, None)
}
