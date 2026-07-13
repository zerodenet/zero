use std::future::Future;

use zero_core::Address;
use zero_engine::EngineError;
use zero_platform_tokio::{TokioDatagramSocket, TokioSocket};
use zero_traits::SocketAddress;

use crate::MeteredStream;

mod inbound;
mod leaf;
mod model;
mod tcp;
mod upstream;

pub use inbound::{
    inbound_acceptor_from_protocol, inbound_acceptor_from_users, setup_inbound_udp_association,
};
pub use model::{
    OwnedSocks5InboundAcceptor, Socks5InboundUdpAssociationHandler,
    Socks5InboundUdpAssociationSetup, Socks5ManagedUdpAssociationTarget,
    Socks5ManagedUdpFlowConfig, Socks5ManagedUdpFlowPlan, Socks5ManagedUdpPacketPathCarrierBuild,
    Socks5ManagedUdpPacketPathCarrierDescriptor, Socks5ManagedUdpPacketPathPlan,
    Socks5TransportLeaf, Socks5UpstreamAssociationCloseReason,
};
pub use tcp::{apply_socks5_tcp_relay_hop, establish_socks5_tcp_connect};
pub use upstream::{
    establish_packet_path_udp_association, establish_registered_udp_association,
    Socks5UdpAssociationRuntime, Socks5UpstreamUdpAssociation,
};

pub fn udp_association_target_from_config(
    tag: &str,
    server: &str,
    port: u16,
    username: Option<&str>,
    password: Option<&str>,
) -> Socks5ManagedUdpAssociationTarget {
    Socks5ManagedUdpFlowConfig::new(tag, server, port, username, password).association_target()
}

pub fn udp_packet_path_carrier_descriptor_from_config(
    tag: &str,
    server: &str,
    port: u16,
    username: Option<&str>,
    password: Option<&str>,
) -> Socks5ManagedUdpPacketPathCarrierDescriptor {
    Socks5ManagedUdpFlowConfig::new(tag, server, port, username, password)
        .packet_path_carrier_descriptor()
}

pub fn udp_packet_path_carrier_build_from_config(
    tag: &str,
    server: &str,
    port: u16,
    username: Option<&str>,
    password: Option<&str>,
) -> Socks5ManagedUdpPacketPathCarrierBuild {
    Socks5ManagedUdpFlowConfig::new(tag, server, port, username, password)
        .packet_path_carrier_build()
}

pub async fn open_socks5_udp_association_target<
    OpenControl,
    OpenControlFut,
    ResolveRelay,
    ResolveRelayFut,
    RecordControl,
    OnClose,
>(
    target: Socks5ManagedUdpAssociationTarget,
    open_control: OpenControl,
    resolve_relay: ResolveRelay,
    record_control: RecordControl,
    on_close: OnClose,
) -> Result<Socks5UpstreamUdpAssociation, EngineError>
where
    OpenControl: FnOnce(&str, u16) -> OpenControlFut,
    OpenControlFut: Future<Output = Result<TokioSocket, EngineError>>,
    ResolveRelay: FnOnce(Address, u16) -> ResolveRelayFut,
    ResolveRelayFut: Future<Output = Result<(SocketAddress, TokioDatagramSocket), EngineError>>,
    RecordControl: FnOnce(&mut MeteredStream<TokioSocket>),
    OnClose: Fn(Socks5UpstreamAssociationCloseReason) + Send + Sync + 'static,
{
    Socks5UpstreamUdpAssociation::establish(
        target.into_protocol_target(),
        open_control,
        resolve_relay,
        record_control,
        on_close,
    )
    .await
}

pub async fn open_socks5_udp_packet_path_build<
    OpenControl,
    OpenControlFut,
    ResolveRelay,
    ResolveRelayFut,
    RecordControl,
    OnClose,
>(
    build: Socks5ManagedUdpPacketPathCarrierBuild,
    open_control: OpenControl,
    resolve_relay: ResolveRelay,
    record_control: RecordControl,
    on_close: OnClose,
) -> Result<Socks5UpstreamUdpAssociation, EngineError>
where
    OpenControl: FnOnce(&str, u16) -> OpenControlFut,
    OpenControlFut: Future<Output = Result<TokioSocket, EngineError>>,
    ResolveRelay: FnOnce(Address, u16) -> ResolveRelayFut,
    ResolveRelayFut: Future<Output = Result<(SocketAddress, TokioDatagramSocket), EngineError>>,
    RecordControl: FnOnce(&mut MeteredStream<TokioSocket>),
    OnClose: Fn(Socks5UpstreamAssociationCloseReason) + Send + Sync + 'static,
{
    open_socks5_udp_association_target(
        Socks5ManagedUdpAssociationTarget::new(
            socks5::udp::packet_path_carrier_association_target(build.into_protocol_build()),
        ),
        open_control,
        resolve_relay,
        record_control,
        on_close,
    )
    .await
}
