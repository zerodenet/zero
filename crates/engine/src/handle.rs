use std::sync::{mpsc, Arc, Mutex};

use mpsc::SyncSender;
use zero_api::{
    ApiResult, CommandRequest, CommandResponse, CommandService, EventFilter, EventSource,
    QueryRequest, QueryResponse, QueryService, RawApiEvent,
};

use super::runtime::Engine;

/// In-process handle providing `QueryService`, `CommandService`, and
/// `EventSource` access to the engine.
///
/// Multiple subscribers can receive events concurrently via
/// `EventSource::subscribe`.  Subscribers that fall behind are dropped
/// transparently (the channel capacity is 64).
#[derive(Clone)]
pub struct EngineHandle {
    engine: Engine,
    subscribers: Arc<Mutex<Vec<SyncSender<RawApiEvent>>>>,
}

impl EngineHandle {
    pub fn new(engine: Engine) -> Self {
        Self {
            engine,
            subscribers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Convenience constructor that loads configuration from `path`.
    pub fn from_path(path: impl AsRef<std::path::Path>) -> Result<Self, super::error::EngineError> {
        Engine::from_path(path).map(Self::new)
    }

    /// Access the underlying `Engine`.
    pub fn inner(&self) -> &Engine {
        &self.engine
    }

    /// Push an event to all active subscribers.
    ///
    /// Stale subscribers (those whose receivers have been dropped) are
    /// removed lazily.
    pub fn emit(&self, event: RawApiEvent) {
        let mut subscribers = self.subscribers.lock().unwrap_or_else(|e| e.into_inner());
        subscribers.retain(|tx| tx.send(event.clone()).is_ok());
    }
}

impl QueryService for EngineHandle {
    fn query(&self, request: QueryRequest) -> ApiResult<QueryResponse> {
        self.engine.query(request)
    }
}

impl CommandService for EngineHandle {
    fn execute(&self, command: CommandRequest) -> ApiResult<CommandResponse> {
        self.engine.execute(command)
    }
}

impl EventSource for EngineHandle {
    type Stream = EventSubscriber;

    fn subscribe(&self, filter: EventFilter) -> ApiResult<Self::Stream> {
        let (tx, rx) = mpsc::sync_channel(64);
        self.subscribers
            .lock()
            .expect("subscriber lock poisoned")
            .push(tx);
        Ok(EventSubscriber { rx, filter })
    }

    fn latest(&self, limit: usize, filter: EventFilter) -> ApiResult<Vec<RawApiEvent>> {
        self.engine.latest(limit, filter)
    }
}

/// A single event subscriber created by `EngineHandle::subscribe`.
///
/// Iterate over it with `recv()` / `try_recv()` to read events. Only
/// events matching the subscriber's filter are yielded; others are
/// silently skipped.
pub struct EventSubscriber {
    rx: mpsc::Receiver<RawApiEvent>,
    filter: EventFilter,
}

impl EventSubscriber {
    /// Block until the next matching event arrives.
    ///
    /// Returns `None` when the publisher has been dropped (the
    /// `EngineHandle` no longer exists).
    pub fn recv(&self) -> Option<RawApiEvent> {
        loop {
            let event = self.rx.recv().ok()?;
            if matches_event(&event, &self.filter) {
                return Some(event);
            }
        }
    }

    /// Non-blocking read of the next matching event.
    pub fn try_recv(&self) -> Option<RawApiEvent> {
        loop {
            let event = self.rx.try_recv().ok()?;
            if matches_event(&event, &self.filter) {
                return Some(event);
            }
        }
    }
}

fn matches_event(event: &RawApiEvent, filter: &EventFilter) -> bool {
    if !filter.event_types.is_empty()
        && !filter
            .event_types
            .iter()
            .any(|event_type| event_type == &event.event_type)
    {
        return false;
    }
    true
}
