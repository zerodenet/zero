use std::marker::PhantomData;

#[cfg(feature = "managed-datagram-runtime")]
use super::super::super::cache::ManagedDatagramConnectionCache;
#[cfg(feature = "managed-datagram-runtime")]
use super::super::super::cache::ManagedUdpConnectionCache;

#[cfg(feature = "managed-datagram-runtime")]
pub(crate) struct ManagedDatagramFlowManager<T, C> {
    pub(super) upstreams: ManagedUdpConnectionCache,
    pub(super) connector: C,
    pub(super) establish_stage: &'static str,
    pub(super) mismatch_stage: &'static str,
    pub(super) mismatch_message: &'static str,
    _resume: PhantomData<T>,
}

#[cfg(feature = "managed-datagram-runtime")]
pub(crate) struct ManagedDatagramSocketFlowManager<T, C> {
    pub(super) upstreams: ManagedDatagramConnectionCache,
    pub(super) connector: C,
    pub(super) establish_stage: &'static str,
    pub(super) send_stage: &'static str,
    pub(super) mismatch_stage: &'static str,
    pub(super) mismatch_message: &'static str,
    _resume: PhantomData<T>,
}

#[cfg(feature = "managed-datagram-runtime")]
impl<T, C> ManagedDatagramFlowManager<T, C> {
    pub(crate) fn new(
        connector: C,
        establish_stage: &'static str,
        mismatch_stage: &'static str,
        mismatch_message: &'static str,
    ) -> Self {
        Self {
            upstreams: ManagedUdpConnectionCache::new(),
            connector,
            establish_stage,
            mismatch_stage,
            mismatch_message,
            _resume: PhantomData,
        }
    }
}

#[cfg(feature = "managed-datagram-runtime")]
impl<T, C> ManagedDatagramSocketFlowManager<T, C> {
    pub(crate) fn new(
        connector: C,
        establish_stage: &'static str,
        send_stage: &'static str,
        mismatch_stage: &'static str,
        mismatch_message: &'static str,
    ) -> Self {
        Self {
            upstreams: ManagedDatagramConnectionCache::new(),
            connector,
            establish_stage,
            send_stage,
            mismatch_stage,
            mismatch_message,
            _resume: PhantomData,
        }
    }
}
