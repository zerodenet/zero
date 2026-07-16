use crate::groups::{UrlTestRuntime, DEFAULT_PROBE_URL};

use super::super::util::parse_ip_address;
use super::super::ProxyHandle;
use super::runtime::with_current_runtime;

pub(super) fn execute_diagnostics_probe_outbound(
    handle: &ProxyHandle,
    cmd: &zero_api::command::DiagnosticsProbeOutboundCommand,
) -> zero_api::ApiResult<zero_api::CommandResponse> {
    let proxy = handle.proxy.clone();
    let target_tag = cmd.target_tag.clone();
    let url = cmd
        .url
        .clone()
        .unwrap_or_else(|| DEFAULT_PROBE_URL.to_owned());

    with_current_runtime(
        "no tokio runtime available for probe_outbound command",
        |rt| {
            rt.block_on(async move {
                match UrlTestRuntime::new(proxy.tcp_runtime_services())
                    .probe_outbound_single(&target_tag, &url)
                    .await
                {
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
            })
        },
    )
}

pub(super) fn execute_diagnostics_dns_cache(
    handle: &ProxyHandle,
    cmd: &zero_api::DiagnosticsDnsCacheCommand,
) -> zero_api::ApiResult<zero_api::CommandResponse> {
    let proxy = handle.proxy.clone();
    let domain = cmd.domain.clone();
    let limit = cmd.limit.unwrap_or(256);

    with_current_runtime("no tokio runtime available for dns_cache command", |rt| {
        rt.block_on(async move {
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
        })
    })
}

pub(super) fn execute_diagnostics_fakeip_lookup(
    handle: &ProxyHandle,
    cmd: &zero_api::DiagnosticsFakeipLookupCommand,
) -> zero_api::ApiResult<zero_api::CommandResponse> {
    let proxy = handle.proxy.clone();
    let domain = cmd.domain.clone();
    let ip = cmd.ip.clone();

    with_current_runtime(
        "no tokio runtime available for fakeip_lookup command",
        |rt| {
            rt.block_on(async move {
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
                            ));
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
            })
        },
    )
}
