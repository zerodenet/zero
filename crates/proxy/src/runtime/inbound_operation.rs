use std::future::Future;
use std::pin::Pin;

use tokio::sync::watch;
use zero_engine::EngineError;

use crate::protocol_registry::BoundInbound;
use crate::runtime::Proxy;

pub(crate) trait PreparedInboundListenerOperation: Send {
    fn execute(
        self: Box<Self>,
        proxy: Proxy,
        bound: BoundInbound,
        shutdown: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<(), EngineError>> + Send + 'static>>;
}

pub(crate) struct InboundListenerOperation<F> {
    run: F,
}

impl<F> InboundListenerOperation<F> {
    pub(crate) fn new(run: F) -> Self {
        Self { run }
    }
}

impl<F, Fut> PreparedInboundListenerOperation for InboundListenerOperation<F>
where
    F: FnOnce(Proxy, BoundInbound, watch::Receiver<bool>) -> Fut + Send + 'static,
    Fut: Future<Output = Result<(), EngineError>> + Send + 'static,
{
    fn execute(
        self: Box<Self>,
        proxy: Proxy,
        bound: BoundInbound,
        shutdown: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<(), EngineError>> + Send + 'static>> {
        Box::pin((self.run)(proxy, bound, shutdown))
    }
}
