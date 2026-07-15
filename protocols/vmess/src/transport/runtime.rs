use std::path::Path;

use zero_traits::{
    ClientTlsProfile, GrpcTransportProfile, ServerTlsProfile, WebSocketTransportProfile,
};

use super::inbound::VmessInboundListenerRequest;
use super::leaf::VmessOutboundLeaf;
use super::options::VmessOutboundOptionsRef;
use crate::inbound::BorrowedVmessInboundUserConfigParts;

#[derive(Debug, Clone, Default)]
pub struct VmessTransportRuntime {
    mux_pool: crate::mux::VmessMuxConnectionPool,
}

impl VmessTransportRuntime {
    pub fn on_config_reloaded(&self) {
        self.mux_pool.evict_all();
    }

    pub fn build_inbound_listener_request<'a, I, TTls, TWs, TGrpc>(
        &self,
        source_dir: Option<&Path>,
        users: I,
        tls: Option<&TTls>,
        ws: Option<&TWs>,
        grpc: Option<&TGrpc>,
    ) -> Result<VmessInboundListenerRequest, zero_transport::RuntimeError>
    where
        I: IntoIterator<Item = BorrowedVmessInboundUserConfigParts<'a>>,
        TTls: ServerTlsProfile + ?Sized,
        TWs: WebSocketTransportProfile + ?Sized,
        TGrpc: GrpcTransportProfile + ?Sized,
    {
        let profile = crate::inbound::VmessInboundProfile::from_config_users(users)?;
        VmessInboundListenerRequest::from_profile_refs(source_dir, profile, tls, ws, grpc)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn build_outbound_leaf<TTls, TWs, TGrpc>(
        &self,
        source_dir: Option<&Path>,
        tag: &str,
        server: &str,
        port: u16,
        options: VmessOutboundOptionsRef<'_>,
        tls: Option<&TTls>,
        ws: Option<&TWs>,
        grpc: Option<&TGrpc>,
    ) -> Result<VmessOutboundLeaf, zero_core::Error>
    where
        TTls: ClientTlsProfile + ?Sized,
        TWs: WebSocketTransportProfile + ?Sized,
        TGrpc: GrpcTransportProfile + ?Sized,
    {
        VmessOutboundLeaf::from_profile_refs(
            source_dir,
            tag,
            server,
            port,
            options.id,
            options.cipher,
            options.mux_concurrency,
            tls,
            ws,
            grpc,
            self.mux_pool.clone(),
        )
    }
}
