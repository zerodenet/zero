use super::*;

impl UdpDispatch {
    /// Send via SOCKS5 upstream association, establishing one if needed.
    pub(crate) async fn send_socks5(
        &mut self,
        proxy: &Proxy,
        tag: &str,
        server: &str,
        port: u16,
        username: Option<&str>,
        password: Option<&str>,
        session: &Session,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        use crate::logging::log_udp_upstream_association_dropped;
        use crate::runtime::socks5_udp::{
            send_socks5_udp_packet, Socks5UdpAssociation, UpstreamAssociationCloseReason,
        };

        let association = Socks5UdpAssociation {
            tag: tag.to_owned(),
            server: server.to_owned(),
            port,
            auth: username
                .zip(password)
                .map(|(u, p)| (u.to_owned(), p.to_owned())),
        };

        match send_socks5_udp_packet(
            proxy,
            &self.inbound_tag,
            &association,
            session,
            payload,
            &mut self.socks5_upstream,
            &mut self.socks5_idle_deadline,
        )
        .await
        {
            Ok(sent) => {
                // packet_sent already recorded in send_socks5_udp_packet
                Ok(sent)
            }
            Err(error) => {
                if let Some(assoc) = self.socks5_upstream.take() {
                    let outbound_tag = assoc.outbound_tag().to_owned();
                    let (svr, p) = assoc.upstream_endpoint();
                    let svr = svr.to_owned();
                    assoc.close(UpstreamAssociationCloseReason::Dropped);
                    log_udp_upstream_association_dropped(
                        &self.inbound_tag,
                        &outbound_tag,
                        &svr,
                        p,
                        &error,
                    );
                }
                self.socks5_idle_deadline = None;
                proxy.record_udp_upstream_send_failure();
                Err(error)
            }
        }
    }
}
