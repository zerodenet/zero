#[cfg(feature = "managed-datagram-runtime")]
mod datagram;
mod key;
#[cfg(any(
    feature = "managed-stream-runtime",
    feature = "managed-datagram-runtime"
))]
mod stream;

#[cfg(feature = "managed-datagram-runtime")]
pub(crate) use datagram::ManagedDatagramConnectionCache;
#[cfg(any(
    feature = "managed-stream-runtime",
    feature = "managed-datagram-runtime"
))]
pub(crate) use stream::ManagedUdpConnectionCache;
