use std::vec::Vec;

use zero_core::{InboundUdpAssociationDispatcher, InboundUdpDispatch};
use zero_engine::EngineError;

use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_ingress::UdpIngressRuntime;
use crate::transport::StreamTraffic;

pub(super) enum UdpAssociationDispatchOutcome {
    ClientHandled,
    PeerResponse {
        sender: zero_traits::SocketAddress,
        payload: Vec<u8>,
    },
    UnexpectedSender {
        sender: zero_traits::SocketAddress,
    },
}

pub(super) struct UdpAssociationDispatchBridge<'a> {
    runtime: &'a UdpIngressRuntime,
    dispatch: &'a mut UdpDispatch,
    pending_control_traffic: &'a mut StreamTraffic,
    source_addr: std::net::SocketAddr,
    outcome: UdpAssociationDispatchOutcome,
}

impl<'a> UdpAssociationDispatchBridge<'a> {
    pub(super) fn new(
        runtime: &'a UdpIngressRuntime,
        dispatch: &'a mut UdpDispatch,
        pending_control_traffic: &'a mut StreamTraffic,
        source_addr: std::net::SocketAddr,
    ) -> Self {
        Self {
            runtime,
            dispatch,
            pending_control_traffic,
            source_addr,
            outcome: UdpAssociationDispatchOutcome::ClientHandled,
        }
    }

    pub(super) fn dispatch(&self) -> &UdpDispatch {
        &*self.dispatch
    }

    pub(super) fn take_outcome(&mut self) -> UdpAssociationDispatchOutcome {
        std::mem::replace(
            &mut self.outcome,
            UdpAssociationDispatchOutcome::ClientHandled,
        )
    }
}

impl InboundUdpAssociationDispatcher for UdpAssociationDispatchBridge<'_> {
    type Error = EngineError;

    async fn dispatch_local_dns(&mut self, domain: &str) -> Result<(), Self::Error> {
        self.runtime.resolve_local_dns(domain).await;
        Ok(())
    }

    async fn dispatch_inbound_packet(
        &mut self,
        inbound_dispatch: InboundUdpDispatch,
        protocol_overhead_bytes: u64,
    ) -> Result<(), Self::Error> {
        let session_id = self
            .runtime
            .dispatch_inbound_packet(
                self.dispatch,
                &inbound_dispatch,
                None,
                Some(self.source_addr),
            )
            .await?;

        self.runtime
            .services()
            .record_session_inbound_traffic(session_id, *self.pending_control_traffic);
        *self.pending_control_traffic = StreamTraffic::default();
        self.runtime
            .services()
            .record_session_inbound_rx(session_id, protocol_overhead_bytes);

        Ok(())
    }

    async fn dispatch_peer_response(
        &mut self,
        sender: zero_traits::SocketAddress,
        payload: &[u8],
    ) -> Result<(), Self::Error> {
        self.outcome = UdpAssociationDispatchOutcome::PeerResponse {
            sender,
            payload: payload.to_vec(),
        };
        Ok(())
    }

    async fn dispatch_unexpected_sender(
        &mut self,
        sender: zero_traits::SocketAddress,
    ) -> Result<(), Self::Error> {
        self.outcome = UdpAssociationDispatchOutcome::UnexpectedSender { sender };
        Ok(())
    }
}
