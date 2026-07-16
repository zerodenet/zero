use tokio::task::JoinSet;
#[cfg(feature = "socks5")]
use tokio::time::Instant as TokioInstant;
use zero_platform_tokio::TokioDatagramSocket;

use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::sessions::CompletedUdpFlow;
#[cfg(feature = "socks5")]
use crate::runtime::udp_flow::state::UpstreamUdpPoll;

#[cfg(feature = "socks5")]
pub(crate) struct UpstreamAssociationView<'a> {
    pub(crate) outbound_tag: &'a str,
}

#[cfg(feature = "socks5")]
pub(crate) struct ClosedUpstreamAssociation {
    pub(crate) outbound_tag: String,
    pub(crate) server: String,
    pub(crate) port: u16,
}

impl UdpDispatch {
    /// Borrow direct socket and chain_tasks for `select!` polling.
    #[cfg(any(
        feature = "hysteria2",
        feature = "shadowsocks",
        all(
            not(feature = "socks5"),
            any(
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            )
        )
    ))]
    pub(crate) fn poll_sockets(&mut self) -> (&TokioDatagramSocket, &mut JoinSet<ChainTask>) {
        (&self.direct_socket, self.flow_state.chain_tasks())
    }

    /// Borrow all polling sources simultaneously for `select!` loops.
    #[cfg(feature = "socks5")]
    pub(crate) fn poll_refs(
        &mut self,
    ) -> (
        &TokioDatagramSocket,
        UpstreamUdpPoll<'_>,
        Option<TokioInstant>,
        &mut JoinSet<ChainTask>,
    ) {
        let (upstream_udp, upstream_idle_deadline, chain_tasks) = self.flow_state.poll_refs();
        (
            &self.direct_socket,
            upstream_udp,
            upstream_idle_deadline,
            chain_tasks,
        )
    }

    /// View of the active upstream association, if established.
    #[cfg(feature = "socks5")]
    pub(crate) fn upstream_association_view(&self) -> Option<UpstreamAssociationView<'_>> {
        self.flow_state
            .upstream_association_view()
            .map(|association| UpstreamAssociationView {
                outbound_tag: association.outbound_tag,
            })
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn touch_upstream_idle(&mut self, timeout: std::time::Duration) {
        self.flow_state.touch_upstream_idle(timeout);
    }

    /// Drop the active upstream association after a receive error.
    #[cfg(feature = "socks5")]
    pub(crate) fn drop_upstream_association(&mut self) -> Option<ClosedUpstreamAssociation> {
        self.flow_state
            .drop_upstream_association()
            .map(|closed| ClosedUpstreamAssociation {
                outbound_tag: closed.outbound_tag,
                server: closed.server,
                port: closed.port,
            })
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn drop_idle_upstream_association(&mut self) -> Option<ClosedUpstreamAssociation> {
        self.flow_state
            .close_idle_upstream()
            .map(|closed| ClosedUpstreamAssociation {
                outbound_tag: closed.outbound_tag,
                server: closed.server,
                port: closed.port,
            })
    }

    /// Finish all tracked flows and close upstreams.
    pub(crate) fn finish_all(mut self) -> Vec<CompletedUdpFlow> {
        #[cfg(feature = "socks5")]
        self.flow_state.close_all_upstreams();

        self.flows.finish_all()
    }
}
