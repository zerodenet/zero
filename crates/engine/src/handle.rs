use std::collections::VecDeque;
use std::sync::{mpsc, Mutex};

use zero_api::{
    ApiResult, CommandRequest, CommandResponse, CommandService, EventFilter, EventSource,
    QueryRequest, QueryResponse, QueryService, RawApiEvent,
};

use super::runtime::Engine;

/// In-process handle providing `QueryService`, `CommandService`, and
/// `EventSource` access to the engine.
///
/// Multiple subscribers can receive events concurrently via
/// `EventSource::subscribe`. Slow subscribers retain their registration;
/// the bounded live queue can shed samples while the event-log sequence
/// still exposes the gap to consumers.
#[derive(Clone)]
pub struct EngineHandle {
    engine: Engine,
}

impl EngineHandle {
    pub fn new(engine: Engine) -> Self {
        Self { engine }
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
        self.engine.emit_event(event);
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
        let (tx, rx) = mpsc::sync_channel(1024);
        self.engine.subscribe_events(tx);
        let initial = if wants_flow_snapshot(&filter) {
            VecDeque::from([self.engine.flow_snapshot_event()])
        } else {
            VecDeque::new()
        };
        Ok(EventSubscriber {
            initial: Mutex::new(initial),
            rx,
            filter,
        })
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
    initial: Mutex<VecDeque<RawApiEvent>>,
    rx: mpsc::Receiver<RawApiEvent>,
    filter: EventFilter,
}

impl EventSubscriber {
    /// Block until the next matching event arrives.
    ///
    /// Returns `None` when the publisher has been dropped (the
    /// `EngineHandle` no longer exists).
    pub fn recv(&self) -> Option<RawApiEvent> {
        if let Some(event) = self
            .initial
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .pop_front()
        {
            return Some(event);
        }
        loop {
            let event = self.rx.recv().ok()?;
            if matches_event(&event, &self.filter) {
                return Some(event);
            }
        }
    }

    /// Non-blocking read of the next matching event.
    pub fn try_recv(&self) -> Option<RawApiEvent> {
        if let Some(event) = self
            .initial
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .pop_front()
        {
            return Some(event);
        }
        loop {
            let event = self.rx.try_recv().ok()?;
            if matches_event(&event, &self.filter) {
                return Some(event);
            }
        }
    }
}

fn wants_flow_snapshot(filter: &EventFilter) -> bool {
    filter.event_types.is_empty()
        || filter.event_types.iter().any(|event_type| {
            matches!(
                event_type.as_str(),
                zero_api::event_type::FLOW_STARTED
                    | zero_api::event_type::FLOW_ROUTED
                    | zero_api::event_type::FLOW_UPDATED
                    | zero_api::event_type::FLOW_COMPLETED
                    | zero_api::event_type::FLOW_SNAPSHOT
            )
        })
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
