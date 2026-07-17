use std::sync::Arc;

use zero_dns::DnsSystem;

use crate::inventory::ProtocolInventory;

/// Narrow network service exposed to protocol-owned connect/handshake code.
/// It deliberately carries no engine, configuration, health, or accounting
/// access.
#[derive(Clone)]
pub(crate) struct UpstreamConnectServices {
    pub(super) resolver: Arc<DnsSystem>,
    pub(super) protocols: ProtocolInventory,
}

impl UpstreamConnectServices {
    pub(super) fn new(resolver: Arc<DnsSystem>, protocols: ProtocolInventory) -> Self {
        Self {
            resolver,
            protocols,
        }
    }

    pub(crate) async fn connect_upstream_owned(
        &self,
        server: String,
        port: u16,
    ) -> Result<zero_platform_tokio::TokioSocket, zero_transport::RuntimeError> {
        self.protocols
            .direct_connector()
            .connect_host(&server, port, self.resolver.as_ref())
            .await
            .map_err(Into::into)
    }

    pub(crate) async fn connect_upstream(
        &self,
        server: &str,
        port: u16,
    ) -> Result<zero_platform_tokio::TokioSocket, zero_transport::RuntimeError> {
        self.connect_upstream_owned(server.to_owned(), port).await
    }
}
