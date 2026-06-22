use super::*;
use std::net::SocketAddr;

use crate::runtime::udp_associate::sessions::CompletedUdpFlow;

impl UdpDispatch {
    /// Create a new dispatcher with an ephemeral direct socket.
    pub(crate) async fn new(inbound_tag: &str) -> Result<Self, EngineError> {
        let direct_socket = TokioDatagramSocket::bind("0.0.0.0:0").await?;
        Ok(Self {
            inbound_tag: inbound_tag.to_owned(),
            flows: UdpSessionFlows::default(),
            direct_socket,
            socks5_upstream: None,
            socks5_idle_deadline: None,
            vless_manager: VlessUdpOutboundManager::new(),
            #[cfg(feature = "vmess")]
            vmess_manager: VmessUdpOutboundManager::new(),
            vless_handles: HashMap::new(),
            #[cfg(feature = "vmess")]
            vmess_handles: HashMap::new(),
            chain_tasks: JoinSet::new(),
            #[cfg(feature = "shadowsocks")]
            ss_manager: SsChainManager::new(),
            #[cfg(feature = "shadowsocks")]
            packet_path_manager: PacketPathManager::new(),
            #[cfg(feature = "trojan")]
            trojan_manager: TrojanChainManager::new(),
            #[cfg(feature = "mieru")]
            mieru_manager: MieruChainManager::new(),
            #[cfg(feature = "hysteria2")]
            h2_manager: H2ChainManager::new(),
        })
    }

    /// Create a new dispatcher with a pre-bound direct socket.
    #[allow(dead_code)]
    pub(crate) fn with_socket(inbound_tag: &str, direct_socket: TokioDatagramSocket) -> Self {
        Self {
            inbound_tag: inbound_tag.to_owned(),
            flows: UdpSessionFlows::default(),
            direct_socket,
            socks5_upstream: None,
            socks5_idle_deadline: None,
            vless_manager: VlessUdpOutboundManager::new(),
            #[cfg(feature = "vmess")]
            vmess_manager: VmessUdpOutboundManager::new(),
            vless_handles: HashMap::new(),
            #[cfg(feature = "vmess")]
            vmess_handles: HashMap::new(),
            chain_tasks: JoinSet::new(),
            #[cfg(feature = "shadowsocks")]
            ss_manager: SsChainManager::new(),
            #[cfg(feature = "shadowsocks")]
            packet_path_manager: PacketPathManager::new(),
            #[cfg(feature = "trojan")]
            trojan_manager: TrojanChainManager::new(),
            #[cfg(feature = "mieru")]
            mieru_manager: MieruChainManager::new(),
            #[cfg(feature = "hysteria2")]
            h2_manager: H2ChainManager::new(),
        }
    }

    /// The direct outbound socket. Inbound handlers poll this for direct
    /// responses and use [`direct_response_session_id`] for metering.
    #[allow(dead_code)]
    pub(crate) fn direct_socket(&self) -> &TokioDatagramSocket {
        &self.direct_socket
    }

    /// Borrow direct socket and chain_tasks for `select!` polling.
    pub(crate) fn poll_sockets(&mut self) -> (&TokioDatagramSocket, &mut JoinSet<ChainTask>) {
        (&self.direct_socket, &mut self.chain_tasks)
    }

    /// Borrow all polling sources simultaneously for `select!` loops.
    pub(crate) fn poll_refs(
        &mut self,
    ) -> (
        &TokioDatagramSocket,
        Option<&crate::protocol_runtime::socks5_udp::ActiveUpstreamSocks5UdpAssociation>,
        Option<TokioInstant>,
        &mut JoinSet<ChainTask>,
    ) {
        (
            &self.direct_socket,
            self.socks5_upstream.as_ref(),
            self.socks5_idle_deadline,
            &mut self.chain_tasks,
        )
    }

    /// The SOCKS5 upstream association, if established.
    #[allow(dead_code)]
    pub(crate) fn socks5_upstream(
        &self,
    ) -> Option<&crate::protocol_runtime::socks5_udp::ActiveUpstreamSocks5UdpAssociation> {
        self.socks5_upstream.as_ref()
    }

    /// The SOCKS5 idle deadline.
    #[allow(dead_code)]
    pub(crate) fn socks5_idle_deadline(&self) -> Option<TokioInstant> {
        self.socks5_idle_deadline
    }

    /// Update the SOCKS5 idle deadline (called after each send / recv).
    #[allow(dead_code)]
    pub(crate) fn touch_socks5_idle(&mut self, timeout: std::time::Duration) {
        self.socks5_idle_deadline = Some(TokioInstant::now() + timeout);
    }

    /// Look up the session ID for a direct response sender.
    pub(crate) fn direct_response_session_id(&self, sender: SocketAddr) -> Option<u64> {
        self.flows.direct_response_session_id(sender)
    }

    /// Look up a session ID by target+port only, regardless of outbound type.
    pub(crate) fn session_id_by_target(
        &self,
        target: &Address,
        port: u16,
        client_session_id: Option<u64>,
    ) -> Option<u64> {
        self.flows
            .session_id_by_target(target, port, client_session_id)
    }

    /// Look up the session ID for an upstream response (requires outbound tag).
    pub(crate) fn upstream_response_session_id(
        &self,
        outbound_tag: &str,
        target: &Address,
        port: u16,
    ) -> Option<u64> {
        self.flows
            .upstream_response_session_id(outbound_tag, target, port)
    }

    /// Take and close the SOCKS5 upstream association (for idle timeout / error).
    #[allow(dead_code)]
    pub(crate) fn take_socks5_upstream(
        &mut self,
    ) -> Option<crate::protocol_runtime::socks5_udp::ActiveUpstreamSocks5UdpAssociation> {
        self.socks5_idle_deadline = None;
        self.socks5_upstream.take()
    }

    /// Close the SOCKS5 upstream association on idle timeout.
    #[allow(dead_code)]
    pub(crate) fn close_socks5_idle(&mut self) {
        use crate::logging::log_udp_upstream_association_idle_timeout;
        use crate::protocol_runtime::socks5_udp::UpstreamAssociationCloseReason;

        if let Some(assoc) = self.socks5_upstream.take() {
            let outbound_tag = assoc.outbound_tag().to_owned();
            let (server, port) = assoc.upstream_endpoint();
            let server = server.to_owned();
            assoc.close(UpstreamAssociationCloseReason::IdleTimeout);
            log_udp_upstream_association_idle_timeout(
                &self.inbound_tag,
                &outbound_tag,
                &server,
                port,
                std::time::Duration::default(),
            );
            self.socks5_idle_deadline = None;
        }
    }

    /// Finish all tracked flows and close upstreams.
    pub(crate) fn finish_all(mut self) -> Vec<CompletedUdpFlow> {
        if let Some(assoc) = self.socks5_upstream {
            use crate::protocol_runtime::socks5_udp::UpstreamAssociationCloseReason;
            assoc.close(UpstreamAssociationCloseReason::Closed);
        }

        for (_key, (session, mut handle)) in self.vless_handles.drain() {
            if let Some(record) = handle.finish(SessionOutcome::ChainedRelayed) {
                log_session_finished(&record, None);
                let _ = session;
            }
        }

        #[cfg(feature = "vmess")]
        for (_key, (session, mut handle)) in self.vmess_handles.drain() {
            if let Some(record) = handle.finish(SessionOutcome::ChainedRelayed) {
                log_session_finished(&record, None);
                let _ = session;
            }
        }

        self.flows.finish_all()
    }
}
