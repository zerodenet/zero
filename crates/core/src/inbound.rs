use alloc::vec::Vec;
use core::future::Future;

use crate::Error;
use zero_traits::AsyncSocket;

pub trait InboundClientResponse<S>: Send + Sync
where
    S: AsyncSocket,
{
    fn send_ok(&self, client: &mut S) -> impl Future<Output = Result<(), Error>> + Send;

    fn send_blocked(&self, client: &mut S) -> impl Future<Output = Result<(), Error>> + Send;

    fn send_upstream_failure(
        &self,
        client: &mut S,
    ) -> impl Future<Output = Result<(), Error>> + Send;
}

pub trait InboundFallbackCapture {
    type Stream;

    fn into_fallback_replay_parts(self) -> (Self::Stream, Vec<u8>);
}
