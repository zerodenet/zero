use std::fmt;

use async_trait::async_trait;

/// A protocol adapter registered in the proxy.
///
/// Implementations are behind `#[cfg(feature = "...")]` gates so only
/// compiled-in protocols appear in the registry.
#[async_trait]
pub(crate) trait ProtocolAdapter: Send + Sync + fmt::Debug {}
