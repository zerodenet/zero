use super::*;

#[cfg(feature = "mixed")]
#[derive(Debug)]
pub(crate) struct MixedAdapter;

#[cfg(feature = "mixed")]
#[async_trait]
impl ProtocolAdapter for MixedAdapter {
    fn name(&self) -> &'static str {
        "mixed"
    }

    fn feature_name(&self) -> &'static str {
        "mixed"
    }

    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        matches!(c, InboundProtocolConfig::Mixed { .. })
    }

    fn supports_outbound(&self, _: &OutboundProtocolConfig) -> bool {
        false
    }

    fn has_inbound(&self) -> bool {
        true
    }

    fn has_outbound(&self) -> bool {
        false
    }

    fn spawn_inbound(
        &self,
        proxy: &Proxy,
        inbound: InboundConfig,
        bound: BoundInbound,
        shutdown_rx: tokio::sync::watch::Receiver<bool>,
        listeners: &mut tokio::task::JoinSet<Result<(), EngineError>>,
    ) {
        let p = proxy.clone();
        listeners.spawn(async move {
            crate::inbound::run_mixed_listener_with_bound(
                &p,
                inbound,
                bound.into_tcp(),
                shutdown_rx,
            )
            .await
        });
    }
}

#[cfg(feature = "mixed")]
impl ProtocolMetadata for MixedAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        protocol_descriptor("mixed", "mixed")
    }
}
