use super::*;

#[cfg(feature = "http_connect")]
#[derive(Debug)]
pub(crate) struct HttpConnectAdapter;

#[cfg(feature = "http_connect")]
impl ProtocolAdapter for HttpConnectAdapter {
    fn name(&self) -> &'static str {
        "http_connect"
    }

    fn feature_name(&self) -> &'static str {
        "http_connect"
    }

    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        matches!(c, InboundProtocolConfig::HttpConnect)
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
            p.run_http_connect_listener_with_bound(inbound, bound.into_tcp(), shutdown_rx)
                .await
        });
    }
}

#[cfg(feature = "http_connect")]
impl ProtocolMetadata for HttpConnectAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        ::http_connect::HttpConnectProtocol.descriptor()
    }
}
