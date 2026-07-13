use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};

use zero_core::Address;
use zero_engine::EngineError;
use zero_platform_tokio::{TokioDatagramSocket, TokioSocket};
use zero_traits::SocketAddress;

use super::{
    open_socks5_udp_association_target, open_socks5_udp_packet_path_build,
    Socks5ManagedUdpAssociationTarget, Socks5ManagedUdpPacketPathCarrierBuild,
    Socks5UpstreamAssociationCloseReason,
};
use crate::MeteredStream;

pub struct Socks5UpstreamUdpAssociation {
    close_recorded: AtomicBool,
    on_close: Box<dyn Fn(Socks5UpstreamAssociationCloseReason) + Send + Sync>,
    association: socks5::udp::Socks5EstablishedUdpAssociation<
        MeteredStream<TokioSocket>,
        TokioDatagramSocket,
    >,
}

#[async_trait::async_trait]
pub trait Socks5UdpAssociationRuntime: Clone + Send + Sync + 'static {
    async fn open_control_socket(
        &self,
        server: &str,
        port: u16,
    ) -> Result<TokioSocket, EngineError>;

    async fn resolve_udp_relay(
        &self,
        relay_address: Address,
        relay_port: u16,
    ) -> Result<(SocketAddress, TokioDatagramSocket), EngineError>;

    fn record_control_traffic(&self, session_id: u64, control: &mut MeteredStream<TokioSocket>);

    fn record_close(&self, reason: Socks5UpstreamAssociationCloseReason);
}

pub async fn establish_registered_udp_association<R>(
    runtime: R,
    target: Socks5ManagedUdpAssociationTarget,
    session_id: u64,
) -> Result<Socks5UpstreamUdpAssociation, EngineError>
where
    R: Socks5UdpAssociationRuntime,
{
    let open_runtime = runtime.clone();
    let resolve_runtime = runtime.clone();
    let record_runtime = runtime.clone();
    open_socks5_udp_association_target(
        target,
        move |server, port| {
            let runtime = open_runtime.clone();
            let server = server.to_owned();
            async move { runtime.open_control_socket(&server, port).await }
        },
        move |relay_address, relay_port| {
            let runtime = resolve_runtime.clone();
            async move { runtime.resolve_udp_relay(relay_address, relay_port).await }
        },
        move |control| record_runtime.record_control_traffic(session_id, control),
        move |reason| runtime.record_close(reason),
    )
    .await
}

pub async fn establish_packet_path_udp_association<R>(
    runtime: R,
    build: Socks5ManagedUdpPacketPathCarrierBuild,
    session_id: u64,
) -> Result<Socks5UpstreamUdpAssociation, EngineError>
where
    R: Socks5UdpAssociationRuntime,
{
    let open_runtime = runtime.clone();
    let resolve_runtime = runtime.clone();
    let record_runtime = runtime.clone();
    open_socks5_udp_packet_path_build(
        build,
        move |server, port| {
            let runtime = open_runtime.clone();
            let server = server.to_owned();
            async move { runtime.open_control_socket(&server, port).await }
        },
        move |relay_address, relay_port| {
            let runtime = resolve_runtime.clone();
            async move { runtime.resolve_udp_relay(relay_address, relay_port).await }
        },
        move |control| record_runtime.record_control_traffic(session_id, control),
        move |reason| runtime.record_close(reason),
    )
    .await
}

impl Socks5UpstreamUdpAssociation {
    pub async fn establish<
        OpenControl,
        OpenControlFut,
        ResolveRelay,
        ResolveRelayFut,
        RecordControl,
        OnClose,
    >(
        target: socks5::udp::Socks5UdpAssociationTarget,
        open_control: OpenControl,
        resolve_relay: ResolveRelay,
        record_control: RecordControl,
        on_close: OnClose,
    ) -> Result<Self, EngineError>
    where
        OpenControl: FnOnce(&str, u16) -> OpenControlFut,
        OpenControlFut: Future<Output = Result<TokioSocket, EngineError>>,
        ResolveRelay: FnOnce(Address, u16) -> ResolveRelayFut,
        ResolveRelayFut: Future<Output = Result<(SocketAddress, TokioDatagramSocket), EngineError>>,
        RecordControl: FnOnce(&mut MeteredStream<TokioSocket>),
        OnClose: Fn(Socks5UpstreamAssociationCloseReason) + Send + Sync + 'static,
    {
        let association = target
            .establish_with_transport(
                |server, port| {
                    let server = server.to_owned();
                    async move {
                        let control = open_control(&server, port).await?;
                        Ok::<_, EngineError>(MeteredStream::new(control))
                    }
                },
                resolve_relay,
                record_control,
            )
            .await?;
        Ok(Self {
            close_recorded: AtomicBool::new(false),
            on_close: Box::new(on_close),
            association,
        })
    }

    pub fn close(self, reason: Socks5UpstreamAssociationCloseReason) {
        self.close_recorded.store(true, Ordering::Relaxed);
        (self.on_close)(reason);
    }

    pub async fn send_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        self.association
            .send_packet(target, port, payload)
            .await
            .map_err(|error| error.into_mapped(EngineError::from))
    }

    pub async fn recv_response_parts(
        &self,
        buf: &mut [u8],
    ) -> Result<(Address, u16, Vec<u8>), EngineError> {
        self.association
            .recv_response_parts(buf)
            .await
            .map_err(|error| error.into_mapped(EngineError::from))
    }

    pub async fn recv_payload(&self, buf: &mut [u8]) -> Result<usize, EngineError> {
        self.association
            .recv_payload(buf)
            .await
            .map_err(|error| error.into_mapped(EngineError::from))
    }
}

impl Drop for Socks5UpstreamUdpAssociation {
    fn drop(&mut self) {
        if !self.close_recorded.load(Ordering::Relaxed) {
            self.close_recorded.store(true, Ordering::Relaxed);
            (self.on_close)(Socks5UpstreamAssociationCloseReason::Closed);
        }
    }
}
