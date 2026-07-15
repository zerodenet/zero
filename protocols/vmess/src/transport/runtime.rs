use std::path::Path;

use zero_traits::{ClientTlsProfile, GrpcTransportProfile, WebSocketTransportProfile};

use super::leaf::VmessOutboundLeaf;
use super::options::VmessOutboundBuildOptionsRef;

#[derive(Debug, Clone, Default)]
pub struct VmessTransportRuntime {
    mux_pool: crate::mux::VmessMuxConnectionPool,
}

impl VmessTransportRuntime {
    pub fn on_config_reloaded(&self) {
        self.mux_pool.evict_all();
    }

    pub fn build_outbound_leaf<TTls, TWs, TGrpc>(
        &self,
        source_dir: Option<&Path>,
        options: VmessOutboundBuildOptionsRef<'_, TTls, TWs, TGrpc>,
    ) -> Result<VmessOutboundLeaf, zero_core::Error>
    where
        TTls: ClientTlsProfile + ?Sized,
        TWs: WebSocketTransportProfile + ?Sized,
        TGrpc: GrpcTransportProfile + ?Sized,
    {
        let VmessOutboundBuildOptionsRef {
            tag,
            server,
            port,
            protocol,
            tls,
            ws,
            grpc,
        } = options;
        VmessOutboundLeaf::from_profile_refs(
            source_dir,
            tag,
            server,
            port,
            protocol.id,
            protocol.cipher,
            protocol.mux_concurrency,
            tls,
            ws,
            grpc,
            self.mux_pool.clone(),
        )
    }
}
