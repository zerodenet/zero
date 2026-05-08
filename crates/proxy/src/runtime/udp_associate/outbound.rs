use tokio::time::Instant as TokioInstant;
use zero_engine::ResolvedLeafOutbound;

use crate::logging::{
    log_udp_upstream_association_created, log_udp_upstream_association_dropped,
    log_udp_upstream_association_reused,
};
use crate::runtime::Proxy;

use super::context::{
    Socks5UdpAssociationEndpoint, Socks5UdpPacketContext, UdpCandidateContext, UdpCandidateFailure,
    UdpCandidateStart,
};
use super::sessions::UdpFlowOutbound;
use super::upstream::{ActiveUpstreamSocks5UdpAssociation, UpstreamAssociationCloseReason};

impl Proxy {
    pub(super) async fn start_udp_flow_candidate(
        &self,
        candidate: ResolvedLeafOutbound<'_>,
        context: UdpCandidateContext<'_>,
    ) -> Result<UdpCandidateStart, UdpCandidateFailure> {
        match candidate {
            ResolvedLeafOutbound::Direct { tag } => {
                let target_addr = self
                    .protocols
                    .direct_outbound
                    .resolve_target_addr(context.session, &self.resolver)
                    .await
                    .map_err(|error| UdpCandidateFailure {
                        stage: "resolve_udp_target",
                        error: error.into(),
                        upstream: None,
                    })?;

                let sent = context
                    .relay
                    .send_to_addr(context.payload, target_addr)
                    .await
                    .map_err(|error| UdpCandidateFailure {
                        stage: "udp_direct_send",
                        error: error.into(),
                        upstream: None,
                    })?;

                Ok(UdpCandidateStart::Flow {
                    outbound: UdpFlowOutbound::Direct {
                        tag: tag.unwrap_or("direct").to_owned(),
                        target_addr,
                    },
                    outbound_tx_bytes: sent as u64,
                })
            }
            ResolvedLeafOutbound::Block { tag } => Ok(UdpCandidateStart::Blocked {
                tag: tag.unwrap_or("block").to_owned(),
            }),
            ResolvedLeafOutbound::Socks5 {
                tag,
                server,
                port,
                username,
                password,
            } => {
                let sent = self
                    .send_socks5_udp_packet(Socks5UdpPacketContext {
                        inbound_tag: context.inbound_tag,
                        tag,
                        server,
                        port,
                        auth: username.zip(password),
                        session: context.session,
                        payload: context.payload,
                        upstream_association: context.upstream_association,
                        upstream_idle_deadline: context.upstream_idle_deadline,
                    })
                    .await?;

                Ok(UdpCandidateStart::Flow {
                    outbound: UdpFlowOutbound::Socks5 {
                        tag: tag.to_owned(),
                        server: server.to_owned(),
                        port,
                        username: username.map(ToOwned::to_owned),
                        password: password.map(ToOwned::to_owned),
                    },
                    outbound_tx_bytes: sent as u64,
                })
            }
            ResolvedLeafOutbound::Vless { server, port, .. } => Err(UdpCandidateFailure {
                stage: "udp_vless_outbound",
                error: zero_core::Error::Unsupported("VLESS UDP outbound is not supported").into(),
                upstream: Some((server.to_owned(), port)),
            }),
        }
    }

    pub(super) async fn send_socks5_udp_packet(
        &self,
        context: Socks5UdpPacketContext<'_>,
    ) -> Result<usize, UdpCandidateFailure> {
        self.ensure_socks5_udp_association(
            context.inbound_tag,
            Socks5UdpAssociationEndpoint {
                tag: context.tag,
                server: context.server,
                port: context.port,
                auth: context.auth,
            },
            context.session.id,
            context.upstream_association,
            context.upstream_idle_deadline,
        )
        .await?;

        let association = context
            .upstream_association
            .as_ref()
            .expect("successful establish stores upstream association");

        let sent = match association
            .send_packet(
                &context.session.target,
                context.session.port,
                context.payload,
            )
            .await
        {
            Ok(sent) => sent,
            Err(error) => {
                self.record_udp_upstream_send_failure();
                if let Some(association) = context.upstream_association.take() {
                    let outbound_tag = association.outbound_tag().to_owned();
                    association.close(UpstreamAssociationCloseReason::Dropped);
                    log_udp_upstream_association_dropped(
                        context.inbound_tag,
                        &outbound_tag,
                        context.server,
                        context.port,
                        &error,
                    );
                }
                *context.upstream_idle_deadline = None;
                return Err(UdpCandidateFailure {
                    stage: "udp_upstream_send",
                    error,
                    upstream: Some((context.server.to_owned(), context.port)),
                });
            }
        };

        self.record_udp_upstream_packet_sent();
        *context.upstream_idle_deadline =
            Some(TokioInstant::now() + self.udp_upstream_idle_timeout());
        Ok(sent)
    }

    async fn ensure_socks5_udp_association(
        &self,
        inbound_tag: &str,
        endpoint: Socks5UdpAssociationEndpoint<'_>,
        session_id: u64,
        upstream_association: &mut Option<ActiveUpstreamSocks5UdpAssociation>,
        upstream_idle_deadline: &mut Option<TokioInstant>,
    ) -> Result<(), UdpCandidateFailure> {
        let needs_new_association = upstream_association
            .as_ref()
            .map(|association| !association.matches(endpoint.tag, endpoint.server, endpoint.port))
            .unwrap_or(true);

        if !needs_new_association {
            self.record_udp_upstream_association_reused();
            log_udp_upstream_association_reused(
                inbound_tag,
                endpoint.tag,
                endpoint.server,
                endpoint.port,
            );
            return Ok(());
        }

        if let Some(association) = upstream_association.take() {
            association.close(UpstreamAssociationCloseReason::Closed);
            *upstream_idle_deadline = None;
        }

        match ActiveUpstreamSocks5UdpAssociation::establish(
            self,
            endpoint.tag,
            endpoint.server,
            endpoint.port,
            endpoint.auth,
            session_id,
        )
        .await
        {
            Ok(association) => {
                self.record_udp_upstream_association_created();
                *upstream_idle_deadline =
                    Some(TokioInstant::now() + self.udp_upstream_idle_timeout());
                log_udp_upstream_association_created(
                    inbound_tag,
                    endpoint.tag,
                    endpoint.server,
                    endpoint.port,
                    self.udp_upstream_idle_timeout(),
                );
                *upstream_association = Some(association);
                Ok(())
            }
            Err(error) => {
                self.record_udp_upstream_association_failed();
                Err(UdpCandidateFailure {
                    stage: "udp_upstream_associate",
                    error,
                    upstream: Some((endpoint.server.to_owned(), endpoint.port)),
                })
            }
        }
    }
}
