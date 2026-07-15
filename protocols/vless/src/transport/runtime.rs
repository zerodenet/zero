use std::path::Path;

use zero_traits::{
    ClientTlsProfile, GrpcTransportProfile, H2TransportProfile, HttpUpgradeTransportProfile,
    SplitHttpTransportProfile, WebSocketTransportProfile,
};

use super::leaf::VlessOutboundLeaf;
use super::options::VlessOutboundBuildOptionsRef;
use super::profile::{VlessQuicClientProfile, VlessRealityClientProfile};

#[derive(Debug, Clone, Default)]
pub struct VlessTransportRuntime {
    mux_pool: crate::mux_pool::MuxConnectionPool,
}

impl VlessTransportRuntime {
    pub fn on_config_reloaded(&self) {
        self.mux_pool.evict_all();
    }

    pub fn build_outbound_leaf<TTls, TWs, TGrpc, TH2, THttp, TSplit>(
        &self,
        source_dir: Option<&Path>,
        options: VlessOutboundBuildOptionsRef<'_, TTls, TWs, TGrpc, TH2, THttp, TSplit>,
    ) -> Result<VlessOutboundLeaf, zero_core::Error>
    where
        TTls: ClientTlsProfile + ?Sized,
        TWs: WebSocketTransportProfile + ?Sized,
        TGrpc: GrpcTransportProfile + ?Sized,
        TH2: H2TransportProfile + ?Sized,
        THttp: HttpUpgradeTransportProfile + ?Sized,
        TSplit: SplitHttpTransportProfile + ?Sized,
    {
        let VlessOutboundBuildOptionsRef {
            tag,
            server,
            port,
            protocol,
            tls,
            ws,
            grpc,
            h2,
            http_upgrade,
            split_http,
        } = options;
        let reality = protocol.reality.map(VlessRealityClientProfile::from);
        let quic = protocol.quic.map(VlessQuicClientProfile::from);
        VlessOutboundLeaf::from_profile_refs(
            source_dir,
            tag,
            server,
            port,
            protocol.id,
            protocol.flow,
            protocol.mux_concurrency,
            tls,
            reality.as_ref(),
            ws,
            grpc,
            h2,
            http_upgrade,
            split_http,
            quic.as_ref(),
            self.mux_pool.clone(),
        )
    }
}
