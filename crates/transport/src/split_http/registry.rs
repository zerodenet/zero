use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use rand::Rng;
use tokio::sync::{oneshot, Mutex};

pub(super) struct SplitHttpPending {
    pub(super) stream: Box<dyn Any + Send>,
    pub(super) _notify: oneshot::Sender<()>,
}

pub(super) fn generate_session_id() -> String {
    let id: u64 = rand::rng().random();
    format!("{id:016x}")
}

pub struct SplitHttpRegistry {
    pub(super) inner: Arc<Mutex<HashMap<String, SplitHttpPending>>>,
}

impl SplitHttpRegistry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Clone for SplitHttpRegistry {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl Default for SplitHttpRegistry {
    fn default() -> Self {
        Self::new()
    }
}
