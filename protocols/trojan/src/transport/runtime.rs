use std::path::Path;

use zero_traits::ServerTlsProfile;

use super::inbound::TrojanInboundListenerRequest;
use super::leaf::TrojanOutboundLeaf;
use super::options::{TrojanInboundOptionsRef, TrojanOutboundBuildOptionsRef};

#[derive(Debug, Clone, Default)]
pub struct TrojanTransportRuntime;

impl TrojanTransportRuntime {
    pub fn build_inbound_listener_request<TTls>(
        &self,
        source_dir: Option<&Path>,
        options: TrojanInboundOptionsRef<'_>,
        tls: Option<&TTls>,
    ) -> Result<TrojanInboundListenerRequest, zero_transport::RuntimeError>
    where
        TTls: ServerTlsProfile + ?Sized,
    {
        TrojanInboundListenerRequest::from_options_refs(source_dir, options, tls)
    }

    pub fn build_outbound_leaf(
        &self,
        source_dir: Option<&Path>,
        options: TrojanOutboundBuildOptionsRef<'_>,
    ) -> Result<TrojanOutboundLeaf, zero_core::Error> {
        Ok(TrojanOutboundLeaf::from_options_refs(source_dir, options))
    }
}
