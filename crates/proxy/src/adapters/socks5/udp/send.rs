use zero_core::Session;
use zero_engine::EngineError;

use super::model::Socks5UdpAssociation;
use super::runtime::Socks5UdpRuntime;
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
        config: resume.owned_association_config(),
    };

    match runtime
        .send_packet(
            request.proxy,
            inbound_tag,
            &association,
            request.session,
            request.payload,
        )
        .await
    {
        Ok(sent) => Ok(sent),
        Err(error) => {
            request.proxy.record_udp_upstream_send_failure();
            Err(error)
        }
    }
}
