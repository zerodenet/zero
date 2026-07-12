use super::model::ProxyHandle;

impl zero_api::QueryService for ProxyHandle {
    fn query(
        &self,
        request: zero_api::QueryRequest,
    ) -> zero_api::ApiResult<zero_api::QueryResponse> {
        if let zero_api::QueryRequest::Capabilities(_) = &request {
            let response = self.inner.query(request)?;
            let zero_api::QueryResponse::Capabilities(mut capabilities) = response else {
                return Ok(response);
            };
            capabilities.protocols = self.proxy.protocols.protocol_capabilities();
            return Ok(zero_api::QueryResponse::Capabilities(capabilities));
        }
        if let zero_api::QueryRequest::TunStatus(_) = &request {
            let info = self.proxy.tun_info.lock().unwrap();
            let snap = match info.as_ref() {
                Some(tun) => zero_api::TunStatusSnapshot {
                    running: true,
                    name: Some(tun.name.clone()),
                    addr: Some(tun.addr.clone()),
                    tag: Some(tun.tag.clone()),
                },
                None => zero_api::TunStatusSnapshot::default(),
            };
            return Ok(zero_api::QueryResponse::TunStatus(snap));
        }
        self.inner.query(request)
    }
}
