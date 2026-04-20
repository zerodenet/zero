use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use super::completed_sessions::CompletedSessionRecord;
use super::error::EngineError;
use super::export::{EngineConfigExport, EngineRuntimeExport, EngineStatusExport};
use super::runtime::Engine;
use super::session_registry::ActiveSession;
use super::stats::EngineStatsSnapshot;

#[derive(Debug)]
pub struct RunningEngine {
    engine: Engine,
    shutdown: Option<oneshot::Sender<()>>,
    task: JoinHandle<Result<(), EngineError>>,
}

impl RunningEngine {
    pub(crate) fn new(
        engine: Engine,
        shutdown: oneshot::Sender<()>,
        task: JoinHandle<Result<(), EngineError>>,
    ) -> Self {
        Self {
            engine,
            shutdown: Some(shutdown),
            task,
        }
    }

    pub fn export_config(&self) -> EngineConfigExport {
        self.engine.export_config()
    }

    pub fn export_runtime(&self) -> EngineRuntimeExport {
        self.engine.export_runtime()
    }

    pub fn export_status(&self) -> EngineStatusExport {
        self.engine.export_status()
    }

    pub fn stats_snapshot(&self) -> EngineStatsSnapshot {
        self.engine.stats_snapshot()
    }

    pub fn active_sessions(&self) -> Vec<ActiveSession> {
        self.engine.active_sessions()
    }

    pub fn completed_sessions(&self) -> Vec<CompletedSessionRecord> {
        self.engine.completed_sessions()
    }

    pub async fn shutdown(mut self) -> Result<(), EngineError> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }

        self.task.await.expect("engine task should join")
    }
}

impl Engine {
    pub fn spawn(self) -> RunningEngine {
        let probe = self.clone();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            self.run_until(async {
                let _ = shutdown_rx.await;
            })
            .await
        });

        RunningEngine::new(probe, shutdown_tx, task)
    }
}
