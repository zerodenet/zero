use std::fmt;

mod build;
mod inbound;
mod metadata;
mod outbound;
mod runtime;
mod support;
mod validation;

/// Registry of all compiled-in protocol adapters.
///
/// Constructed at proxy startup via `build()`. Replaces the manual
/// match arms in `ProtocolInventory::supports_*` and `protocol_name` functions.
#[derive(Clone, Default)]
pub(crate) struct ProtocolRegistry {
    adapters: Vec<std::sync::Arc<dyn crate::protocol_registry::RegisteredProtocolCapability>>,
}

impl fmt::Debug for ProtocolRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProtocolRegistry")
            .field("adapter_count", &self.adapters.len())
            .finish()
    }
}

#[cfg(test)]
mod tests;
