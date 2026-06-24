use std::io;
use std::ops::Deref;

use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use zero_engine::{Engine, EngineError};

use super::Proxy;

pub struct RunningProxy {
    pub(super) proxy: Proxy,
    pub(super) shutdown: Option<oneshot::Sender<()>>,
    pub(super) task: JoinHandle<Result<(), EngineError>>,
}

impl RunningProxy {
    pub fn engine(&self) -> &Engine {
        self.proxy.engine()
    }

    pub async fn shutdown(mut self) -> Result<(), EngineError> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }

        self.task
            .await
            .map_err(|error| EngineError::from(io::Error::other(error)))?
    }
}

impl Deref for RunningProxy {
    type Target = Engine;

    fn deref(&self) -> &Self::Target {
        self.proxy.engine()
    }
}
