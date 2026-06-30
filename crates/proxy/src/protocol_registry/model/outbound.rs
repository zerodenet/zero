use crate::runtime::orchestration::{OutboundEndpoint, TcpPathCategory};

/// Runtime-neutral facts about one resolved outbound leaf.
///
/// The proxy runtime uses this for orchestration decisions without matching on
/// concrete protocol variants. Protocol-private fields remain owned by the
/// adapter that claimed the leaf.
#[derive(Debug, Clone, Copy)]
pub(crate) struct OutboundLeafRuntime<'a> {
    pub(crate) tcp_path: TcpPathCategory,
    pub(crate) health_tag: Option<&'a str>,
    pub(crate) endpoint: Option<OutboundEndpoint<'a>>,
    pub(crate) kernel_tag: Option<&'a str>,
    pub(crate) udp_policy_tag: Option<&'a str>,
}
