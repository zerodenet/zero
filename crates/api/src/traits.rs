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
}

pub trait EventSink {
    fn publish(&self, event: &RawApiEvent) -> ApiResult<PublishResult>;
}

pub trait ApiCodec {
    fn encode<T: Serialize>(&self, value: &T) -> ApiResult<Vec<u8>>;

    fn decode<T: DeserializeOwned>(&self, bytes: &[u8]) -> ApiResult<T>;
}

pub trait ApiAuth {
    fn authorize(&self, context: &AuthContext, required: Permission) -> ApiResult<()>;
}
