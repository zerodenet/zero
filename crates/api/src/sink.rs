use std::collections::HashSet;
use std::io::Write;
use std::sync::Mutex;

use crate::{ApiError, ApiErrorCode, ApiResult, EventSink, PublishResult, RawApiEvent};

pub struct CallbackEventSink<F> {
    name: String,
    callback: F,
}

impl<F> CallbackEventSink<F> {
    pub fn new(name: impl Into<String>, callback: F) -> Self {
        Self {
            name: name.into(),
            callback,
        }
    }
}

impl<F> EventSink for CallbackEventSink<F>
where
    F: Fn(&RawApiEvent) -> ApiResult<PublishResult> + Send + Sync,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn publish(&self, event: &RawApiEvent) -> ApiResult<PublishResult> {
        (self.callback)(event)
    }
}

#[derive(Debug)]
pub struct JsonLineEventSink<W> {
    name: String,
    writer: Mutex<W>,
}

impl<W> JsonLineEventSink<W>
where
    W: Write,
{
    pub fn new(writer: W) -> Self {
        Self {
            name: "jsonl".to_owned(),
            writer: Mutex::new(writer),
        }
    }

    pub fn named(name: impl Into<String>, writer: W) -> Self {
        Self {
            name: name.into(),
            writer: Mutex::new(writer),
        }
    }

    pub fn into_inner(self) -> ApiResult<W> {
        self.writer.into_inner().map_err(|_| {
            ApiError::new(
                ApiErrorCode::Internal,
                "json-line event sink lock was poisoned",
            )
        })
    }
}

impl<W> EventSink for JsonLineEventSink<W>
where
    W: Write + Send,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn publish(&self, event: &RawApiEvent) -> ApiResult<PublishResult> {
        let mut writer = self.writer.lock().map_err(|_| {
            ApiError::new(
                ApiErrorCode::Internal,
                "json-line event sink lock was poisoned",
            )
        })?;

        serde_json::to_writer(&mut *writer, event).map_err(serialization_error)?;
        writer.write_all(b"\n").map_err(io_error)?;
        writer.flush().map_err(io_error)?;

        Ok(PublishResult::delivered())
    }
}

fn serialization_error(error: serde_json::Error) -> ApiError {
    ApiError {
        code: ApiErrorCode::Internal,
        message: "failed to serialize event as json line".to_owned(),
        field_path: None,
        cause: Some(error.to_string()),
    }
}

fn io_error(error: std::io::Error) -> ApiError {
    ApiError {
        code: ApiErrorCode::Internal,
        message: "failed to write event sink output".to_owned(),
        field_path: None,
        cause: Some(error.to_string()),
    }
}
