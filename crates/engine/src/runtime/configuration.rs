use std::io;
use std::path::Path;
use std::sync::Arc;

use tracing::{info, warn};
use zero_config::RuntimeConfig;

use super::Engine;
use crate::{EngineError, EnginePlan};

impl Engine {
    /// Rebuild and atomically install the route/config plan, then notify runtime subscribers.
    pub fn reload_config(&self, new_config: RuntimeConfig) -> Result<(), EngineError> {
        let new_router = Arc::new(new_config.route.compile(new_config.source_dir())?);
        let new_plan = Arc::new(EnginePlan::build(&new_config)?);

        *self
            .router
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = new_router;
        *self.plan.lock().unwrap_or_else(|error| error.into_inner()) = new_plan;
        *self.mode.lock().unwrap_or_else(|error| error.into_inner()) = new_config.mode.clone();

        let config_for_persist = new_config.clone();
        *self.config.write().expect("config lock poisoned") = Arc::new(new_config);
        self.passive_relay_health.clear();
        self.event_log.push_config_changed();

        if let Some(path) = &self.config_path {
            if let Err(error) = write_config_to_file(path, &config_for_persist) {
                warn!(%error, path = %path.display(), "failed to persist config after reload");
            } else {
                info!(path = %path.display(), "config persisted");
            }
        }

        for sender in self
            .reload_notify
            .lock()
            .expect("reload notify lock poisoned")
            .iter()
        {
            let _ = sender.send(());
        }
        Ok(())
    }

    pub fn subscribe_reload(&self) -> std::sync::mpsc::Receiver<()> {
        let (sender, receiver) = std::sync::mpsc::channel();
        self.reload_notify
            .lock()
            .expect("reload notify lock poisoned")
            .push(sender);
        receiver
    }
}

fn write_config_to_file(path: &Path, config: &RuntimeConfig) -> Result<(), io::Error> {
    let json = serde_json::to_string_pretty(config).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("serialize config: {error}"),
        )
    })?;
    std::fs::write(path, json)
}
