use crate::groups::DEFAULT_PROBE_URL;
use crate::runtime::Proxy;

/// Wraps [`EngineHandle`] with TUN command interception.
///
/// TUN start/stop commands are handled by the proxy runtime,
/// not the engine. This wrapper intercepts those commands
/// before they reach `EngineHandle`.
#[derive(Clone)]
pub struct ProxyHandle {
    inner: zero_engine::EngineHandle,
    proxy: Proxy,
}

impl ProxyHandle {
    pub fn new(inner: zero_engine::EngineHandle, proxy: Proxy) -> Self {
        Self { inner, proxy }
    }

    /// Access the underlying EngineHandle.
    pub fn engine_handle(&self) -> &zero_engine::EngineHandle {
        &self.inner
    }
}

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

impl zero_api::CommandService for ProxyHandle {
    fn execute(
        &self,
        command: zero_api::CommandRequest,
    ) -> zero_api::ApiResult<zero_api::CommandResponse> {
        match &command {
            zero_api::CommandRequest::TunStart(cmd) => {
                let proxy = self.proxy.clone();
                let name = cmd.name.clone();
                let addr = cmd.addr.clone();
                let mask = cmd.mask.clone();
                let mtu = cmd.mtu;
                let tag = cmd.tag.clone();
                match tokio::runtime::Handle::try_current() {
                    Ok(rt) => rt.block_on(async move {
                        proxy
                            .start_tun(name.as_deref(), &addr, &mask, mtu, &tag)
                            .await
                            .map(|_| zero_api::CommandResponse::accepted())
                            .map_err(|e| {
                                zero_api::ApiError::new(
                                    zero_api::ApiErrorCode::Internal,
                                    e.to_string(),
                                )
                            })
                    }),
                    Err(_) => Err(zero_api::ApiError::new(
                        zero_api::ApiErrorCode::Internal,
                        "no tokio runtime available for TUN command",
                    )),
                }
            }
            zero_api::CommandRequest::TunStop(_) => match tokio::runtime::Handle::try_current() {
                Ok(rt) => rt.block_on(async move {
                    self.proxy
                        .stop_tun()
                        .map(|_| zero_api::CommandResponse::accepted())
                        .map_err(|e| {
                            zero_api::ApiError::new(zero_api::ApiErrorCode::Internal, e.to_string())
                        })
                }),
                Err(_) => Err(zero_api::ApiError::new(
                    zero_api::ApiErrorCode::Internal,
                    "no tokio runtime available for TUN command",
                )),
            },
            zero_api::CommandRequest::DiagnosticsProbeOutbound(cmd) => {
                let proxy = self.proxy.clone();
                let target_tag = cmd.target_tag.clone();
                let url = cmd
                    .url
                    .clone()
                    .unwrap_or_else(|| DEFAULT_PROBE_URL.to_owned());
                match tokio::runtime::Handle::try_current() {
                    Ok(rt) => rt.block_on(async move {
                        match proxy.probe_outbound_single(&target_tag, &url).await {
                            Ok(latency_ms) => Ok(zero_api::CommandResponse {
                                accepted: true,
                                result: Some(serde_json::json!({
                                    "target_tag": target_tag,
                                    "url": url,
                                    "via": "through_proxy",
                                    "reachable": true,
                                    "latency_ms": latency_ms,
                                })),
                            }),
                            Err(error) => Ok(zero_api::CommandResponse {
                                accepted: true,
                                result: Some(serde_json::json!({
                                    "target_tag": target_tag,
                                    "url": url,
                                    "via": "through_proxy",
                                    "reachable": false,
                                    "latency_ms": null,
                                    "error": error.to_string(),
                                })),
                            }),
                        }
                    }),
                    Err(_) => Err(zero_api::ApiError::new(
                        zero_api::ApiErrorCode::Internal,
                        "no tokio runtime available for probe_outbound command",
                    )),
                }
            }
            zero_api::CommandRequest::DiagnosticsDnsCache(cmd) => {
                let proxy = self.proxy.clone();
                let domain = cmd.domain.clone();
                let limit = cmd.limit.unwrap_or(256);
                match tokio::runtime::Handle::try_current() {
                    Ok(rt) => rt.block_on(async move {
                        let resolver = &proxy.resolver;
                        let enabled = resolver.cache_enabled();
                        let result = if let Some(domain) = domain {
                            match resolver.inspect_cache(&domain).await {
                                Some((addresses, ttl_seconds)) => serde_json::json!({
                                    "enabled": enabled,
                                    "domain": domain,
                                    "hit": true,
                                    "addresses": addresses,
                                    "ttl_seconds": ttl_seconds,
                                }),
                                None => serde_json::json!({
                                    "enabled": enabled,
                                    "domain": domain,
                                    "hit": false,
                                    "addresses": [],
                                    "ttl_seconds": null,
                                }),
                            }
                        } else {
                            let entries: Vec<_> = resolver
                                .list_cache(limit)
                                .await
                                .into_iter()
                                .map(|(domain, addresses, ttl_seconds)| {
                                    serde_json::json!({
                                        "domain": domain,
                                        "addresses": addresses,
                                        "ttl_seconds": ttl_seconds,
                                    })
                                })
                                .collect();
                            let count = entries.len();
                            serde_json::json!({
                                "enabled": enabled,
                                "entries": entries,
                                "count": count,
                            })
                        };
                        Ok(zero_api::CommandResponse {
                            accepted: true,
                            result: Some(result),
                        })
                    }),
                    Err(_) => Err(zero_api::ApiError::new(
                        zero_api::ApiErrorCode::Internal,
                        "no tokio runtime available for dns_cache command",
                    )),
                }
            }
            zero_api::CommandRequest::DiagnosticsFakeipLookup(cmd) => {
                let proxy = self.proxy.clone();
                let domain = cmd.domain.clone();
                let ip = cmd.ip.clone();
                match tokio::runtime::Handle::try_current() {
                    Ok(rt) => rt.block_on(async move {
                        let resolver = &proxy.resolver;
                        let enabled = resolver.fake_ip_enabled();
                        let result = if let Some(domain) = domain {
                            let fake_ip = resolver.lookup_fake_ip_domain(&domain).await;
                            serde_json::json!({
                                "enabled": enabled,
                                "domain": domain,
                                "fake_ip": fake_ip,
                            })
                        } else if let Some(ip) = ip {
                            let domain = match parse_ip_address(&ip) {
                                Some(addr) => resolver.lookup_fake_ip(&addr).await,
                                None => {
                                    return Err(zero_api::ApiError::new(
                                        zero_api::ApiErrorCode::InvalidArgument,
                                        format!("invalid ip `{ip}`"),
                                    ))
                                }
                            };
                            serde_json::json!({
                                "enabled": enabled,
                                "ip": ip,
                                "domain": domain,
                            })
                        } else {
                            return Err(zero_api::ApiError::new(
                                zero_api::ApiErrorCode::InvalidArgument,
                                "fakeip_lookup requires `domain` or `ip`",
                            ));
                        };
                        Ok(zero_api::CommandResponse {
                            accepted: true,
                            result: Some(result),
                        })
                    }),
                    Err(_) => Err(zero_api::ApiError::new(
                        zero_api::ApiErrorCode::Internal,
                        "no tokio runtime available for fakeip_lookup command",
                    )),
                }
            }
            _ => self.inner.execute(command),
        }
    }
}

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

fn parse_ip_address(s: &str) -> Option<zero_traits::IpAddress> {
    match s.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(v4)) => Some(zero_traits::IpAddress::V4(v4.octets())),
        Ok(std::net::IpAddr::V6(v6)) => Some(zero_traits::IpAddress::V6(v6.octets())),
        Err(_) => None,
    }
}
