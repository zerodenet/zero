use zero_core::Session;
use zero_engine::EngineError;

use tokio::time::Instant as TokioInstant;

use super::active::ActiveUpstreamSocks5UdpAssociation;
use super::model::{Socks5UdpAssociation, UpstreamAssociationCloseReason};
use super::runtime::Socks5UdpRuntime;
use crate::logging::{
    log_udp_upstream_association_created, log_udp_upstream_association_dropped,
    log_udp_upstream_association_reused,
};
use crate::runtime::udp_flow::managed::ManagedUdpFlowResume;
use crate::runtime::Proxy;

pub(super) struct Socks5UdpSend<'a> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) tag: &'a str,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ManagedUdpFlowResume,
    pub(crate) session: &'a Session,
    pub(crate) payload: &'a [u8],
}

pub(crate) async fn send(
    request: Socks5UdpSend<'_>,
    inbound_tag: &str,
    runtime: &mut Socks5UdpRuntime,
) -> Result<usize, EngineError> {
    let Some(resume) = request.resume.as_ref::<socks5::Socks5UdpFlowResume>() else {
        return Err(EngineError::Io(std::io::Error::other(
            "expected SOCKS5 UDP flow resume",
        )));
    };
    let association = Socks5UdpAssociation {
        outbound_tag: request.tag.to_owned(),
        server: request.server.to_owned(),
        port: request.port,
        auth: resume
            .username()
            .zip(resume.password())
            .map(|(u, p)| (u.to_owned(), p.to_owned())),
    };

    match send_socks5_udp_packet(
        request.proxy,
        inbound_tag,
        &association,
        request.session,
        request.payload,
        &mut runtime.upstream,
        &mut runtime.idle_deadline,
    )
    .await
    {
        Ok(sent) => Ok(sent),
        Err(error) => {
            if let Some(assoc) = runtime.upstream.take() {
                let outbound_tag = assoc.outbound_tag().to_owned();
                let (server, port) = assoc.upstream_endpoint();
                let server = server.to_owned();
                assoc.close(UpstreamAssociationCloseReason::Dropped);
                log_udp_upstream_association_dropped(
                    inbound_tag,
                    &outbound_tag,
                    &server,
                    port,
                    &error,
                );
            }
            runtime.idle_deadline = None;
            request.proxy.record_udp_upstream_send_failure();
            Err(error)
        }
    }
}

async fn ensure_socks5_udp_association(
    proxy: &Proxy,
    inbound_tag: &str,
    association: &Socks5UdpAssociation,
    session_id: u64,
    upstream_association: &mut Option<ActiveUpstreamSocks5UdpAssociation>,
    upstream_idle_deadline: &mut Option<TokioInstant>,
) -> Result<(), EngineError> {
    let needs_new_association = upstream_association
        .as_ref()
        .map(|a| {
            !a.matches(
                &association.outbound_tag,
                &association.server,
                association.port,
            )
        })
        .unwrap_or(true);

    if !needs_new_association {
        proxy.record_udp_upstream_association_reused();
        log_udp_upstream_association_reused(
            inbound_tag,
            &association.outbound_tag,
            &association.server,
            association.port,
        );
        return Ok(());
    }

    if let Some(a) = upstream_association.take() {
        a.close(UpstreamAssociationCloseReason::Closed);
        *upstream_idle_deadline = None;
    }

    match ActiveUpstreamSocks5UdpAssociation::establish(
        proxy,
        &association.outbound_tag,
        &association.server,
        association.port,
        association
            .auth
            .as_ref()
            .map(|(u, p)| (u.as_str(), p.as_str())),
        session_id,
    )
    .await
    {
        Ok(a) => {
            proxy.record_udp_upstream_association_created();
            *upstream_idle_deadline = Some(TokioInstant::now() + proxy.udp_upstream_idle_timeout());
            log_udp_upstream_association_created(
                inbound_tag,
                &association.outbound_tag,
                &association.server,
                association.port,
                proxy.udp_upstream_idle_timeout(),
            );
            *upstream_association = Some(a);
            Ok(())
        }
        Err(error) => {
            proxy.record_udp_upstream_association_failed();
            Err(error)
        }
    }
}

async fn send_socks5_udp_packet(
    proxy: &Proxy,
    inbound_tag: &str,
    association: &Socks5UdpAssociation,
    session: &Session,
    payload: &[u8],
    upstream_association: &mut Option<ActiveUpstreamSocks5UdpAssociation>,
    upstream_idle_deadline: &mut Option<TokioInstant>,
) -> Result<usize, EngineError> {
    ensure_socks5_udp_association(
        proxy,
        inbound_tag,
        association,
        session.id,
        upstream_association,
        upstream_idle_deadline,
    )
    .await?;

    let association_ref = upstream_association
        .as_ref()
        .expect("successful establish stores upstream association");

    match association_ref
        .send_packet(&session.target, session.port, payload)
        .await
    {
        Ok(sent) => {
            proxy.record_udp_upstream_packet_sent();
            *upstream_idle_deadline = Some(TokioInstant::now() + proxy.udp_upstream_idle_timeout());
            Ok(sent)
        }
        Err(error) => {
            proxy.record_udp_upstream_send_failure();
            if let Some(a) = upstream_association.take() {
                let outbound_tag = a.outbound_tag().to_owned();
                a.close(UpstreamAssociationCloseReason::Dropped);
                log_udp_upstream_association_dropped(
                    inbound_tag,
                    &outbound_tag,
                    &association.server,
                    association.port,
                    &error,
                );
            }
            *upstream_idle_deadline = None;
            Err(error)
        }
    }
}
