use tokio::time::Instant as TokioInstant;
use zero_core::Session;
use zero_engine::EngineError;

use super::super::super::contract::{
    UpstreamAssociationCloseReason, UpstreamAssociationTarget, UpstreamAssociationTransport,
};
use super::model::UpstreamAssociationRuntime;
use crate::protocol_registry::UdpRuntimeServices;

impl<T, A> UpstreamAssociationRuntime<T, A>
where
    T: UpstreamAssociationTarget,
    A: UpstreamAssociationTransport<T>,
{
    pub(crate) async fn send_packet(
        &mut self,
        services: &UdpRuntimeServices,
        inbound_tag: &str,
        association: T,
        session: &Session,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        self.ensure_association(services, inbound_tag, association, session.id)
            .await?;

        let association_ref = self
            .upstream
            .association()
            .expect("successful establish stores upstream association");

        match association_ref
            .send_packet(&session.target, session.port, payload)
            .await
        {
            Ok(sent) => {
                services.record_udp_upstream_packet_sent();
                self.idle_deadline =
                    Some(TokioInstant::now() + services.udp_upstream_idle_timeout());
                Ok(sent)
            }
            Err(error) => {
                services.record_udp_upstream_send_failure();
                self.drop_after_send_error(inbound_tag, &error);
                Err(error)
            }
        }
    }

    async fn ensure_association(
        &mut self,
        services: &UdpRuntimeServices,
        inbound_tag: &str,
        association: T,
        session_id: u64,
    ) -> Result<(), EngineError> {
        let needs_new_association = !self.upstream.matches_target(&association);

        if !needs_new_association {
            services.record_udp_upstream_association_reused();
            let (outbound_tag, server, port) = association.log_parts();
            crate::logging::log_udp_upstream_association_reused(
                inbound_tag,
                outbound_tag,
                server,
                port,
            );
            return Ok(());
        }

        if let Some(a) = self.upstream.take() {
            let (_, association) = a.into_parts();
            association.close(UpstreamAssociationCloseReason::Closed);
            self.idle_deadline = None;
        }

        match A::establish(services.clone(), association.clone(), session_id).await {
            Ok(a) => {
                services.record_udp_upstream_association_created();
                self.idle_deadline =
                    Some(TokioInstant::now() + services.udp_upstream_idle_timeout());
                let (outbound_tag, server, port) = association.log_parts();
                crate::logging::log_udp_upstream_association_created(
                    inbound_tag,
                    outbound_tag,
                    server,
                    port,
                    services.udp_upstream_idle_timeout(),
                );
                let _ = self.upstream.insert(association, a);
                Ok(())
            }
            Err(error) => {
                services.record_udp_upstream_association_failed();
                Err(error)
            }
        }
    }

    pub(crate) fn drop_after_send_error(&mut self, inbound_tag: &str, error: &EngineError) {
        if let Some(assoc) = self.upstream.take() {
            let (record, association) = assoc.into_parts();
            association.close(UpstreamAssociationCloseReason::Dropped);
            let (outbound_tag, server, port) = record.log_parts();
            crate::logging::log_udp_upstream_association_dropped(
                inbound_tag,
                outbound_tag,
                server,
                port,
                error,
            );
        }
        self.idle_deadline = None;
    }

    pub(crate) fn close_idle(&mut self) -> Option<T> {
        self.take_upstream().map(|association| {
            let (target, association) = association.into_parts();
            association.close(UpstreamAssociationCloseReason::IdleTimeout);
            target
        })
    }

    pub(crate) fn close_dropped(&mut self) -> Option<T> {
        self.take_upstream().map(|association| {
            let (target, association) = association.into_parts();
            association.close(UpstreamAssociationCloseReason::Dropped);
            target
        })
    }

    pub(crate) fn close_all_upstreams(&mut self) {
        if let Some(association) = self.upstream.take() {
            let (_, association) = association.into_parts();
            association.close(UpstreamAssociationCloseReason::Closed);
        }
        self.idle_deadline = None;
    }
}
