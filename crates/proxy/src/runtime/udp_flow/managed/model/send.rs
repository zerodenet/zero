//! Managed UDP existing-send model facade.
//!
//! The root stays as a facade so datagram, stream-packet, and relay-stream send
//! models do not regrow into one mixed implementation bucket.

#[cfg(feature = "managed-datagram-runtime")]
mod datagram;
#[cfg(feature = "managed-stream-runtime")]
mod relay;
#[cfg(feature = "managed-stream-runtime")]
mod stream;

#[cfg(feature = "managed-datagram-runtime")]
pub(crate) use datagram::ManagedDatagramExistingSend;
#[cfg(feature = "managed-stream-runtime")]
pub(crate) use relay::ManagedRelayExistingSend;
#[cfg(feature = "managed-stream-runtime")]
pub(crate) use stream::ManagedStreamExistingSend;
