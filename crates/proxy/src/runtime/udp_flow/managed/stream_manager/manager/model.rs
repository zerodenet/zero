use std::marker::PhantomData;

use zero_core::Session;

use super::super::super::cache::ManagedUdpConnectionCache;
use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_flow::packet_path::{UdpFlowContext, UdpPacketRef};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

pub(crate) struct ManagedStreamFlowManager<T> {
    pub(super) upstreams: ManagedUdpConnectionCache,
    pub(super) establish_stage: &'static str,
    pub(super) relay_upstream_stage: &'static str,
    pub(super) relay_establish_stage: &'static str,
    pub(super) relay_send_stage: &'static str,
    pub(super) mismatch_stage: &'static str,
    pub(super) mismatch_message: &'static str,
    _resume: PhantomData<T>,
}

pub(super) struct ManagedStreamRelayRequest<'a, T> {
    pub(super) ctx: UdpFlowContext<'a>,
    pub(super) stream: TcpRelayStream,
    pub(super) tls_server_name: Option<&'a str>,
    pub(super) proxy: Option<&'a Proxy>,
    pub(super) session: &'a Session,
    pub(super) endpoint: OutboundEndpoint<'a>,
    pub(super) resume: T,
    pub(super) packet_ref: UdpPacketRef<'a>,
}

impl<T> ManagedStreamFlowManager<T> {
    pub(crate) fn new(
        establish_stage: &'static str,
        relay_upstream_stage: &'static str,
        relay_establish_stage: &'static str,
        relay_send_stage: &'static str,
        mismatch_stage: &'static str,
        mismatch_message: &'static str,
    ) -> Self {
        Self {
            upstreams: ManagedUdpConnectionCache::new(),
            establish_stage,
            relay_upstream_stage,
            relay_establish_stage,
            relay_send_stage,
            mismatch_stage,
            mismatch_message,
            _resume: PhantomData,
        }
    }
}
