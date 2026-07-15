use std::path::Path;

use zero_traits::{ClientTlsProfile, GrpcTransportProfile, WebSocketTransportProfile};

use super::leaf::VmessOutboundLeaf;

#[derive(Debug, Clone, Default)]
pub struct VmessTransportRuntime {
    mux_pool: crate::mux::VmessMuxConnectionPool,
}

impl VmessTransportRuntime {
    pub fn on_config_reloaded(&self) {
        self.mux_pool.evict_all();
    }

    #[allow(clippy::too_many_arguments)]
    pub fn build_outbound_leaf<TTls, TWs, TGrpc>(
        &self,
        source_dir: Option<&Path>,
        tag: &str,
        server: &str,
        port: u16,
        id: &str,
        cipher: &str,
        mux_concurrency: Option<u32>,
        tls: Option<&TTls>,
        ws: Option<&TWs>,
        grpc: Option<&TGrpc>,
    ) -> Result<VmessOutboundLeaf, zero_core::Error>
    where
        TTls: ClientTlsProfile + ?Sized,
        TWs: WebSocketTransportProfile + ?Sized,
        TGrpc: GrpcTransportProfile + ?Sized,
    {
        VmessOutboundLeaf::from_config_refs(
            source_dir,
            tag,
            server,
            port,
            id,
            cipher,
            mux_concurrency,
            tls,
            ws,
            grpc,
            self.mux_pool.clone(),
        )
    }
}
