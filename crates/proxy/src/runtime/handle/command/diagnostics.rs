use crate::groups::UrlTestRuntime;
use zero_traits::{DnsResolver, IpAddress};

use super::super::util::parse_ip_address;
use super::super::ProxyHandle;
use super::runtime::with_current_runtime;

pub(super) fn execute_diagnostics_probe_target(
    handle: &ProxyHandle,
    cmd: &zero_api::DiagnosticsProbeTargetCommand,
) -> zero_api::ApiResult<zero_api::CommandResponse> {
    let proxy = handle.proxy.clone();
    let target_tag = cmd.target_tag.clone();

    with_current_runtime(
        "no tokio runtime available for probe_target command",
        |rt| {
            rt.block_on(async move {
                let Some((host, port)) = probe_target_endpoint(&proxy, &target_tag)? else {
                    return Ok(zero_api::CommandResponse {
                        accepted: true,
                        result: Some(serde_json::json!({
                            "target_tag": target_tag,
                            "reachable": false,
                            "error": "outbound has no probeable fixed server",
                        })),
                    });
                };

                let started = std::time::Instant::now();
                let reachable = matches!(
                    tokio::time::timeout(
                        std::time::Duration::from_secs(2),
                        proxy.protocols.direct_connector().connect_host(
                            &host,
                            port,
                            proxy.resolver.as_ref()
                        ),
                    )
                    .await,
                    Ok(Ok(_))
                );
                Ok(zero_api::CommandResponse {
                    accepted: true,
                    result: Some(serde_json::json!({
                        "target_tag": target_tag,
                        "server": host,
                        "port": port,
                        "reachable": reachable,
                        "latency_ms": reachable.then(|| started.elapsed().as_millis() as u64),
                    })),
                })
            })
        },
    )
}

pub(super) fn execute_diagnostics_dns_lookup(
    handle: &ProxyHandle,
    cmd: &zero_api::DiagnosticsDnsLookupCommand,
) -> zero_api::ApiResult<zero_api::CommandResponse> {
    let proxy = handle.proxy.clone();
    let hostname = cmd.hostname.clone();

    with_current_runtime("no tokio runtime available for dns_lookup command", |rt| {
        rt.block_on(async move {
            let addresses = proxy
                .resolver
                .resolve(&hostname)
                .await
                .map_err(|error| {
                    zero_api::ApiError::new(
                        zero_api::ApiErrorCode::InvalidArgument,
                        format!("failed to resolve `{hostname}`: {error}"),
                    )
                })?
                .into_iter()
                .map(ip_address_string)
                .collect::<Vec<_>>();
            let count = addresses.len();
            Ok(zero_api::CommandResponse {
                accepted: true,
                result: Some(serde_json::json!({
                    "hostname": hostname,
                    "resolved_addresses": addresses,
                    "count": count,
                })),
            })
        })
    })
}

fn probe_target_endpoint(
    proxy: &crate::runtime::Proxy,
    target_tag: &str,
) -> zero_api::ApiResult<Option<(String, u16)>> {
    let plan = proxy.engine().plan();
    let target_id = plan.target_id(target_tag).ok_or_else(|| {
        zero_api::ApiError::new(
            zero_api::ApiErrorCode::NotFound,
            format!("target `{target_tag}` was not found"),
        )
    })?;
    let (resolved, _plan) = proxy.engine().resolve_target_id(target_id).ok_or_else(|| {
        zero_api::ApiError::new(
            zero_api::ApiErrorCode::NotFound,
            format!("target `{target_tag}` could not be resolved"),
        )
    })?;
    let leaf = match &resolved {
        zero_engine::ResolvedOutbound::Single(leaf) => Some(leaf),
        zero_engine::ResolvedOutbound::Fallback { candidates } => candidates.first(),
        zero_engine::ResolvedOutbound::Relay { .. } => None,
    };
    let Some(leaf) = leaf else {
        return Ok(None);
    };
    let runtime = proxy
        .protocols
        .claim_outbound_leaf(proxy.config.as_ref(), leaf.clone())
        .map_err(|error| {
            zero_api::ApiError::new(
                zero_api::ApiErrorCode::InvalidArgument,
                format!("failed to claim probe target `{target_tag}`: {error}"),
            )
        })?
        .runtime();
    Ok(runtime
        .endpoint
        .map(|endpoint| (endpoint.server, endpoint.port)))
}

fn ip_address_string(address: IpAddress) -> String {
    match address {
        IpAddress::V4(bytes) => std::net::Ipv4Addr::from(bytes).to_string(),
        IpAddress::V6(bytes) => std::net::Ipv6Addr::from(bytes).to_string(),
    }
}

pub(super) fn execute_diagnostics_probe_outbound(
    handle: &ProxyHandle,
    cmd: &zero_api::command::DiagnosticsProbeOutboundCommand,
) -> zero_api::ApiResult<zero_api::CommandResponse> {
    let proxy = handle.proxy.clone();
    let target_tag = cmd.target_tag.clone();
    let config = handle.proxy.engine().config();
    let url = config
        .runtime
        .latency_test_url_or(cmd.url.as_deref())
        .to_owned();

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
