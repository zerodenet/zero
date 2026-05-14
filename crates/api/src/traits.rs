use std::collections::HashSet;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::ApiResult;
use crate::{
    AuthContext, CommandRequest, CommandResponse, EventFilter, Permission, PublishResult,
    QueryRequest, QueryResponse, RawApiEvent,
};

pub trait QueryService {
    fn query(&self, request: QueryRequest) -> ApiResult<QueryResponse>;
}

pub trait CommandService {
    fn execute(&self, command: CommandRequest) -> ApiResult<CommandResponse>;
}

pub trait EventSource {
    type Stream;

    fn subscribe(&self, filter: EventFilter) -> ApiResult<Self::Stream>;

    /// Snapshot of recent events matching the filter.
    fn latest(&self, limit: usize, filter: EventFilter) -> ApiResult<Vec<RawApiEvent>>;
}

pub trait EventSink {
    /// Human-readable name for logging and debugging.
    fn name(&self) -> &str;

    /// Publish a single event to this sink.
    fn publish(&self, event: &RawApiEvent) -> ApiResult<PublishResult>;

    /// Flush any buffered events.
    fn flush(&self) -> ApiResult<()> {
        Ok(())
    }

    /// Optional event type filter for this sink. If `None`, accepts all events.
    fn filter(&self) -> &Option<HashSet<String>> {
        &None
    }

    /// Preferred batch size for this sink. Default 1 (no batching).
    fn batch_size(&self) -> usize {
        1
    }
}

pub trait ApiCodec {
    fn encode<T: Serialize>(&self, value: &T) -> ApiResult<Vec<u8>>;

    fn decode<T: DeserializeOwned>(&self, bytes: &[u8]) -> ApiResult<T>;
}

pub trait ApiAuth {
    fn authorize(&self, context: &AuthContext, required: Permission) -> ApiResult<()>;
}
