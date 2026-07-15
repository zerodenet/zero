use std::path::Path;

use zero_traits::{
    ClientTlsProfile, GrpcTransportProfile, H2TransportProfile, HttpUpgradeTransportProfile,
    InboundFallbackProfile, ServerTlsProfile, SplitHttpTransportProfile, WebSocketTransportProfile,
};

use super::inbound::{VlessInboundBindPlan, VlessInboundListenerRequest};
use super::leaf::VlessOutboundLeaf;
use super::profile::{
    VlessQuicBindOptionsRef, VlessQuicBindProfile, VlessQuicClientOptionsRef,
    VlessQuicClientProfile, VlessRealityClientOptionsRef, VlessRealityClientProfile,
    VlessRealityServerOptionsRef,
};
use crate::inbound::BorrowedVlessInboundUserConfigParts;

#[derive(Debug, Clone, Default)]
pub struct VlessTransportRuntime {
    mux_pool: crate::mux_pool::MuxConnectionPool,
}

impl VlessTransportRuntime {
    pub fn on_config_reloaded(&self) {
        self.mux_pool.evict_all();
    }

    pub fn build_inbound_bind_plan(
        &self,
        source_dir: Option<&Path>,
        quic: Option<VlessQuicBindOptionsRef<'_>>,
    ) -> VlessInboundBindPlan {
        let quic = quic.map(VlessQuicBindProfile::from);
        VlessInboundBindPlan::from_quic_profile(source_dir, quic.as_ref())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn build_inbound_listener_request<'a, I, TTls, TWs, TGrpc, TH2, THttp, TSplit, TFallback>(
        &self,
        source_dir: Option<&Path>,
        users: I,
        reality: Option<VlessRealityServerOptionsRef<'a>>,
        tls: Option<&TTls>,
        ws: Option<&TWs>,
        grpc: Option<&TGrpc>,
        h2: Option<&TH2>,
        http_upgrade: Option<&THttp>,
        split_http: Option<&TSplit>,
        fallback: Option<&TFallback>,
    ) -> Result<VlessInboundListenerRequest, zero_transport::RuntimeError>
    where
        I: IntoIterator<Item = BorrowedVlessInboundUserConfigParts<'a>>,
        TTls: ServerTlsProfile + ?Sized,
        TWs: WebSocketTransportProfile + ?Sized,
        TGrpc: GrpcTransportProfile + ?Sized,
        TH2: H2TransportProfile + ?Sized,
        THttp: HttpUpgradeTransportProfile + ?Sized,
        TSplit: SplitHttpTransportProfile + ?Sized,
        TFallback: InboundFallbackProfile + ?Sized,
    {
        let profile = crate::inbound::VlessInboundProfile::from_config_users(users)?;
        let reality = reality.map(crate::reality::VlessRealityServerProfile::from);
        VlessInboundListenerRequest::from_config_refs(
            source_dir,
            profile,
            reality,
            tls,
            ws,
            grpc,
            h2,
            http_upgrade,
            split_http,
            fallback,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn build_outbound_leaf<TTls, TWs, TGrpc, TH2, THttp, TSplit>(
        &self,
        source_dir: Option<&Path>,
        tag: &str,
        server: &str,
        port: u16,
        id: &str,
        flow: Option<&str>,
        mux_concurrency: Option<u32>,
        tls: Option<&TTls>,
        reality: Option<VlessRealityClientOptionsRef<'_>>,
        ws: Option<&TWs>,
        grpc: Option<&TGrpc>,
        h2: Option<&TH2>,
        http_upgrade: Option<&THttp>,
        split_http: Option<&TSplit>,
        quic: Option<VlessQuicClientOptionsRef<'_>>,
    ) -> Result<VlessOutboundLeaf, zero_core::Error>
    where
        TTls: ClientTlsProfile + ?Sized,
        TWs: WebSocketTransportProfile + ?Sized,
        TGrpc: GrpcTransportProfile + ?Sized,
        TH2: H2TransportProfile + ?Sized,
        THttp: HttpUpgradeTransportProfile + ?Sized,
        TSplit: SplitHttpTransportProfile + ?Sized,
    {
        let reality = reality.map(VlessRealityClientProfile::from);
        let quic = quic.map(VlessQuicClientProfile::from);
        VlessOutboundLeaf::from_config_refs(
            source_dir,
            tag,
            server,
            port,
            id,
            flow,
            mux_concurrency,
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
