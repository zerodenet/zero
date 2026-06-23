use super::*;

impl VmessAdapter {
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
            let (users, tls, ws, grpc) = match &inbound.protocol {
                InboundProtocolConfig::Vmess {
                    users,
                    tls,
                    ws,
                    grpc,
                } => (users.clone(), tls.clone(), ws.clone(), grpc.clone()),
                _ => {
                    return Err(EngineError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "vmess adapter received non-vmess inbound config",
                    )));
                }
            };
            crate::inbound::run_vmess_listener_with_bound(
                &p,
                crate::inbound::VmessInboundRequest {
                    inbound,
                    users,
                    tls,
                    ws,
                    grpc,
                },
                bound.into_tcp(),
                shutdown_rx,
            )
            .await
        });
    }
}
