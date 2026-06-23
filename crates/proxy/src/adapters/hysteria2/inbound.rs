use super::*;

impl Hysteria2Adapter {
    pub(super) async fn bind_inbound_impl(
        &self,
        inbound: &InboundConfig,
        source_dir: Option<&std::path::Path>,
    ) -> Result<BoundInbound, EngineError> {
        let listen = format!("{}:{}", inbound.listen.address, inbound.listen.port);
        if let InboundProtocolConfig::Hysteria2 {
            cert_path,
            key_path,
            ..
        } = &inbound.protocol
        {
            let cert = cert_path
                .clone()
                .unwrap_or_else(|| "certs/fullchain.pem".to_string());
            let key = key_path
                .clone()
                .unwrap_or_else(|| "certs/privkey.pem".to_string());
            let endpoint = QuicInbound::bind(&listen, &cert, &key, source_dir).await?;
            Ok(BoundInbound::Quic(endpoint))
        } else {
            unreachable!("hysteria2 adapter only handles Hysteria2 config")
        }
    }

    pub(super) fn spawn_inbound_impl(
        &self,
        proxy: &Proxy,
        inbound: InboundConfig,
        bound: BoundInbound,
        shutdown_rx: tokio::sync::watch::Receiver<bool>,
        listeners: &mut tokio::task::JoinSet<Result<(), EngineError>>,
    ) {
        let p = proxy.clone();
        listeners.spawn(async move {
            crate::inbound::run_hysteria2_listener_with_bound(&p, inbound, bound, shutdown_rx).await
        });
    }
}
