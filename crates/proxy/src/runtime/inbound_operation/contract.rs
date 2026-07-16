use std::future::Future;
use std::pin::Pin;

use tokio::sync::watch;
use zero_engine::EngineError;

use crate::protocol_registry::BoundInbound;
use crate::runtime::route_runtime::InboundListenerRuntime;

pub(crate) trait PreparedInboundListenerOperation: Send {
    fn execute(
        self: Box<Self>,
        runtime: InboundListenerRuntime,
        bound: BoundInbound,
        shutdown: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<(), EngineError>> + Send + 'static>>;
}
