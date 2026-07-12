use super::model::ProxyHandle;

impl zero_api::EventSource for ProxyHandle {
    type Stream = <zero_engine::EngineHandle as zero_api::EventSource>::Stream;

    fn subscribe(&self, filter: zero_api::EventFilter) -> zero_api::ApiResult<Self::Stream> {
        self.inner.subscribe(filter)
    }

    fn latest(
        &self,
        limit: usize,
        filter: zero_api::EventFilter,
    ) -> zero_api::ApiResult<Vec<zero_api::RawApiEvent>> {
        self.inner.latest(limit, filter)
    }
}
