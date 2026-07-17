//! Managed UDP start dispatch facade.
//!
//! The root stays as a facade so request-kind selection, datagram extraction,
//! and stream extraction do not collapse back into one implementation bucket.

#[cfg(feature = "managed-datagram-runtime")]
mod datagram;
#[cfg(feature = "managed-stream-runtime")]
mod relay;
mod request;
#[cfg(feature = "managed-stream-runtime")]
mod stream;
