//! Architectural boundary tests for the proxy data plane.
//!
//! These tests deliberately lock responsibilities and dependency direction,
//! not a particular module layout. Protocol adapters may be folded or split
//! without changing these assertions.

use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("proxy crate must live under crates/")
        .to_path_buf()
}

fn proxy_src() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn rust_sources(root: &Path) -> Vec<PathBuf> {
    let mut sources = Vec::new();
    collect_rust_sources(root, &mut sources);
    sources
}

fn collect_rust_sources(root: &Path, sources: &mut Vec<PathBuf>) {
    for entry in
        fs::read_dir(root).unwrap_or_else(|error| panic!("read {}: {error}", root.display()))
    {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            collect_rust_sources(&path, sources);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            sources.push(path);
        }
    }
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn assert_sources_exclude(root: &Path, forbidden: &[&str]) {
    for path in rust_sources(root) {
        let source = read(&path);
        for token in forbidden {
            assert!(
                !source.contains(token),
                "{} must not contain boundary token `{token}`",
                path.display()
            );
        }
    }
}

#[test]
fn adapters_prepare_protocol_parts_but_do_not_own_runtime_execution() {
    assert_sources_exclude(
        &proxy_src().join("adapters"),
        &[
            "crate::runtime::Proxy",
            "use crate::runtime::Proxy",
            "tokio::spawn",
            "JoinSet",
            "listener_loop",
            "run_tcp_listener_loop",
            "run_logged_tcp_listener_loop",
            "run_logged_quic_listener_loop",
            "serve_inbound",
        ],
    );
}

#[test]
fn runtime_operations_are_protocol_neutral() {
    let operation_files = [
        "runtime/inbound_operation.rs",
        "runtime/tcp_dispatch/operation.rs",
        "runtime/udp_dispatch/operation.rs",
        "runtime/udp_dispatch/packet_path_operation.rs",
    ];
    let protocol_names = [
        "Vless",
        "Vmess",
        "Trojan",
        "Socks5",
        "Shadowsocks",
        "Mieru",
        "Hysteria2",
    ];
    for relative in operation_files {
        let path = proxy_src().join(relative);
        let source = read(&path);
        for protocol in protocol_names {
            assert!(
                !source.contains(protocol),
                "{} must execute capabilities without concrete `{protocol}` types",
                path.display()
            );
        }
    }
}

#[test]
fn tcp_runtime_dispatch_does_not_retain_engine_leaf_types() {
    assert_sources_exclude(
        &proxy_src().join("runtime/tcp_dispatch"),
        &["ResolvedLeafOutbound"],
    );
}

#[test]
fn proxy_runtime_does_not_retain_engine_leaf_types() {
    assert_sources_exclude(&proxy_src().join("runtime"), &["ResolvedLeafOutbound"]);
}

#[test]
fn udp_runtime_dispatch_does_not_retain_engine_leaf_types() {
    assert_sources_exclude(
        &proxy_src().join("runtime/udp_dispatch"),
        &["ResolvedLeafOutbound"],
    );
}

#[test]
fn udp_managed_bridge_runtime_does_not_retain_engine_leaf_types() {
    assert_sources_exclude(
        &proxy_src().join("runtime/udp_flow/managed/bridge"),
        &["ResolvedLeafOutbound"],
    );
}

#[test]
fn udp_packet_path_chain_runtime_does_not_retain_engine_leaf_types() {
    assert_sources_exclude(
        &proxy_src().join("runtime/udp_flow/packet_path_chain"),
        &["ResolvedLeafOutbound"],
    );
}

#[test]
fn transport_integrations_do_not_parse_top_level_inbound_protocol_enum() {
    assert_sources_exclude(
        &workspace_root().join("crates/transport/src"),
        &["InboundProtocolConfig"],
    );
}

#[test]
fn generic_runtime_does_not_dispatch_protocol_config_variants() {
    assert_sources_exclude(
        &proxy_src().join("runtime"),
        &[
            "match InboundProtocolConfig",
            "match &inbound.protocol",
            "match ResolvedLeafOutbound",
            "match leaf {\n        ResolvedLeafOutbound::",
            "ProtocolAdapter",
        ],
    );
}

#[test]
fn listener_and_task_lifecycle_are_runtime_owned() {
    let inbound_operation = read(&proxy_src().join("runtime/inbound_operation.rs"));
    let listener_loop = read(&proxy_src().join("runtime/listener_loop.rs"));
    assert!(inbound_operation.contains("PreparedInboundListenerOperation"));
    assert!(inbound_operation.contains("InboundConnectionContext"));
    assert!(inbound_operation.contains("JoinSet") || listener_loop.contains("JoinSet"));
    assert!(
        inbound_operation.contains("tokio::spawn") || listener_loop.contains("tokio::spawn"),
        "runtime must own connection task fan-out"
    );
}

#[test]
fn capability_surface_is_split_and_context_is_narrow() {
    let capability = read(&proxy_src().join("protocol_registry/capability.rs"));
    let context = read(&proxy_src().join("protocol_registry/context.rs"));
    for capability_name in [
        "ProtocolSupportCapability",
        "InboundListenerCapability",
        "TcpOutboundCapability",
        "UdpFlowCapability",
        "UdpPacketPathCapability",
    ] {
        assert!(capability.contains(&format!("trait {capability_name}")));
    }
    assert!(!capability.contains("trait ProtocolAdapter"));
    assert!(!capability.contains("proxy: &Proxy"));
    assert!(capability.contains("BoundInbound"));
    assert!(capability.contains("fn runtime(&self) -> OutboundLeafRuntime;"));
    assert!(!capability.contains("fn claims_outbound_leaf("));
    assert!(!capability.contains("fn outbound_leaf_runtime("));
    assert!(context.contains(
        "pub(crate) struct OutboundAdapterContext {\n    source_dir: Option<std::path::PathBuf>,"
    ));
    assert!(
        !context.contains("pub(crate) struct OutboundAdapterContext<'a> {\n    proxy: &'a Proxy,")
    );
    assert!(context.contains("pub(crate) struct UdpAdapterContext<'a> {\n    source_dir: Option<&'a std::path::Path>,\n    services: UdpRuntimeServices,"));
    assert!(!context.contains("pub(crate) struct UdpAdapterContext<'a> {\n    proxy: &'a Proxy,"));
    let tcp_leaf = read(&proxy_src().join("inventory/tcp/leaf.rs"));
    let tcp_operation = read(&proxy_src().join("runtime/tcp_dispatch/operation.rs"));
    let udp_operation = read(&proxy_src().join("runtime/udp_dispatch/operation.rs"));
    assert!(tcp_leaf.contains("OutboundAdapterContext"));
    assert!(tcp_operation.contains("TcpRuntimeServices"));
    assert!(udp_operation.contains("UdpAdapterContext"));
}

#[test]
fn transport_bridge_operations_are_generic() {
    let tcp = read(&proxy_src().join("runtime/tcp_dispatch/operation.rs"));
    let udp = rust_sources(&proxy_src().join("protocol_registry/transport_leaf"))
        .into_iter()
        .map(|path| read(&path))
        .collect::<String>();
    assert!(tcp.contains("TransportBridgeTcpConnectOperation"));
    assert!(tcp.contains("TransportBridgeTcpRelayOperation"));
    assert!(tcp.contains("PreparedTransportBridgeLeaf"));
    assert!(udp.contains("prepare_transport_bridge_udp_direct"));
    assert!(udp.contains("prepare_owned_transport_bridge_udp_relay_two_stream"));
}

#[test]
fn transport_bridge_helpers_drop_raw_leaf_resolution_paths() {
    let resolve = read(&proxy_src().join("adapters/transport_bridge.rs"));
    let tcp = read(&proxy_src().join("protocol_registry/transport_leaf/tcp.rs"));
    let udp = read(&proxy_src().join("protocol_registry/transport_leaf/udp.rs"));
    assert!(!resolve.contains("prepare_last_transport_bridge_leaf"));
    assert!(!resolve.contains("prepare_transport_bridge_leaf"));
    assert!(!resolve.contains("trait ProtocolTransportLeafResolver {"));
    assert!(!resolve.contains("trait ProtocolTransportLeafResolver<'a>"));
    assert!(!resolve.contains("ResolvedLeafOutbound"));
    assert!(!tcp.contains("ResolvedLeafOutbound"));
    assert!(!udp.contains("ResolvedLeafOutbound"));
    assert_sources_exclude(
        &proxy_src().join("protocol_registry/transport_leaf"),
        &["ResolvedLeafOutbound"],
    );
}

#[test]
fn tcp_prepared_operations_do_not_borrow_inventory_or_runtime_services() {
    let tcp_leaf = read(&proxy_src().join("inventory/tcp/leaf.rs"));
    let context = read(&proxy_src().join("protocol_registry/context.rs"));
    assert!(tcp_leaf.contains("fn prepare_claimed_tcp_candidate<'a>(\n        &self,"));
    assert!(tcp_leaf.contains("fn prepare_claimed_tcp_relay_hop<'a>(\n        &self,"));
    assert!(context.contains("pub(crate) fn prepare_tcp_outbound<'a>(\n        &self,"));
    assert!(!context.contains("pub(crate) fn prepare_tcp_candidate<'a>(\n        &self,"));
    assert!(!context.contains("pub(crate) fn prepare_tcp_relay_chain<'a>(\n        &self,"));
    assert!(!context.contains("pub(crate) fn prepare_tcp_relay_hop<'a>(\n        &self,"));
    assert!(!context.contains("ResolvedLeafOutbound"));
}

#[test]
fn udp_prepared_operations_do_not_borrow_adapters_or_bridges() {
    let capability = read(&proxy_src().join("protocol_registry/capability.rs"));
    let udp = read(&proxy_src().join("protocol_registry/transport_leaf/udp.rs"));
    assert!(capability.contains("trait ClaimedUdpFlowLeaf<'a>"));
    assert!(capability.contains("fn prepare_udp_flow("));
    assert!(capability.contains("fn prepare_owned_udp_relay_final_hop("));
    assert!(capability.contains("fn prepare_owned_udp_relay_two_stream("));
    assert!(!capability.contains("fn prepare_udp_relay_final_hop<'a>(\n        &self,"));
    assert!(
        !udp.contains("bridge: &'a TBridge"),
        "prepared UDP transport operations must own bridge state instead of borrowing it"
    );
}

#[test]
fn protocol_crates_do_not_depend_on_proxy_or_config_union_crates() {
    let protocols = workspace_root().join("protocols");
    for entry in fs::read_dir(&protocols).expect("read protocols") {
        let crate_dir = entry.expect("protocol crate").path();
        if !crate_dir.is_dir() {
            continue;
        }
        let manifest = crate_dir.join("Cargo.toml");
        if !manifest.exists() {
            continue;
        }
        let source = read(&manifest);
        for forbidden in ["zero-proxy", "zero-config"] {
            assert!(
                !source.contains(forbidden),
                "{} must not depend on `{forbidden}`",
                manifest.display()
            );
        }
    }
}

#[test]
fn protocol_registration_is_the_only_compiled_adapter_collection() {
    let register = read(&proxy_src().join("register.rs"));
    let inventory = rust_sources(&proxy_src().join("inventory"))
        .into_iter()
        .map(|path| read(&path))
        .collect::<String>();
    assert!(register.contains("ProtocolRegistry"));
    for protocol in ["VlessAdapter", "VmessAdapter", "TrojanAdapter"] {
        assert!(register.contains(protocol));
        assert!(
            !inventory.contains(protocol),
            "inventory must dispatch capabilities without concrete `{protocol}` access"
        );
    }
}

#[test]
fn runtime_owns_post_accept_route_execution() {
    let inbound_route = proxy_src().join("runtime/inbound_route");
    let combined = rust_sources(&inbound_route)
        .into_iter()
        .map(|path| read(&path))
        .collect::<String>();
    assert!(combined.contains("dispatch_protocol_stream_route"));
    assert!(combined.contains("dispatch_protocol_mux_route"));
    assert!(combined.contains("recorded"));
    assert!(combined.contains("InboundRouteRuntime"));
    assert!(combined.contains("MuxSubstreamRuntime"));
    assert!(!combined.contains("run_udp: move |proxy: Proxy"));
    assert!(!combined.contains("run_mux: move |proxy: Proxy"));
}

#[test]
fn adapter_runtime_service_access_does_not_expose_proxy() {
    let context = read(&proxy_src().join("protocol_registry/context.rs"));
    assert!(context.contains("struct TcpRuntimeServices"));
    assert!(context.contains("struct UdpRuntimeServices"));
    assert!(context.contains("fn runtime_services"));
    let adapters = rust_sources(&proxy_src().join("adapters"))
        .into_iter()
        .map(|path| read(&path))
        .collect::<String>();
    assert!(!adapters.contains("ctx.proxy()"));
    assert!(!adapters.contains("Proxy {"));
}

#[test]
fn protocol_adapter_roots_do_not_construct_transport_plans_or_request_bundles_inline() {
    for relative in [
        "adapters/vless.rs",
        "adapters/vmess.rs",
        "adapters/trojan.rs",
    ] {
        let source = read(&proxy_src().join(relative));
        for forbidden in [
            "PreparedVlessOutboundRequestBundle",
            "PreparedVmessOutboundRequestBundle",
            "PreparedTrojanOutboundRequestBundle",
            "VlessOutboundLeaf::from_profile_refs",
            "VmessOutboundLeaf::from_profile_refs",
            "OwnedVlessOutboundTransportPlan::from_profile_refs",
            "OwnedVmessOutboundTransportPlan::from_profile_refs",
            "OwnedTrojanOutboundTlsPlan::from_parts",
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} must delegate protocol-private leaf construction instead of building `{forbidden}` inline"
            );
        }
    }
}

#[test]
fn protocol_adapter_listener_modules_do_not_construct_transport_listener_requests_inline() {
    for relative in [
        "adapters/vless/listener.rs",
        "adapters/vmess/listener.rs",
        "adapters/trojan/listener.rs",
    ] {
        let source = read(&proxy_src().join(relative));
        for forbidden in [
            "VlessInboundListenerRequest::from_profiles",
            "VmessInboundListenerRequest::from_profile_refs",
            "TrojanInboundListenerRequest::new",
            "build_required_tls_acceptor",
            "build_optional_tls_acceptor",
            "OwnedVlessInboundTransportPlan::from_profile_refs",
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} must delegate protocol-owned inbound listener construction instead of building `{forbidden}` inline"
            );
        }
    }
}

#[test]
fn protocol_transport_roots_do_not_reexport_internal_outbound_plan_types() {
    for (relative, forbidden) in [
        (
            "protocols/vless/src/transport.rs",
            "pub use outbound::OwnedVlessOutboundTransportPlan;",
        ),
        (
            "protocols/vmess/src/transport.rs",
            "pub use outbound::OwnedVmessOutboundTransportPlan;",
        ),
        (
            "protocols/trojan/src/transport.rs",
            "pub use outbound::OwnedTrojanOutboundTlsPlan;",
        ),
    ] {
        let source = read(&workspace_root().join(relative));
        assert!(
            !source.contains(forbidden),
            "{relative} must keep transport plan internals behind leaf-owned entrypoints instead of re-exporting `{forbidden}`"
        );
    }
}

#[test]
fn protocol_transport_roots_do_not_reexport_internal_owned_config_intermediates() {
    for (relative, forbidden) in [
        (
            "protocols/vless/src/transport.rs",
            "OwnedVlessInboundListenerConfig",
        ),
        (
            "protocols/vless/src/transport.rs",
            "OwnedVlessOutboundLeafConfig",
        ),
        (
            "protocols/vmess/src/transport.rs",
            "OwnedVmessInboundListenerConfig",
        ),
        (
            "protocols/vmess/src/transport.rs",
            "OwnedVmessOutboundLeafConfig",
        ),
        (
            "protocols/trojan/src/transport.rs",
            "OwnedTrojanInboundListenerConfig",
        ),
        (
            "protocols/trojan/src/transport.rs",
            "OwnedTrojanOutboundLeafConfig",
        ),
    ] {
        let source = read(&workspace_root().join(relative));
        assert!(
            !source.contains(forbidden),
            "{relative} must keep `{forbidden}` internal instead of re-exporting protocol-owned config intermediates"
        );
    }
}

#[test]
fn adapters_do_not_hold_protocol_owned_config_intermediates() {
    for (relative, forbidden) in [
        ("adapters/vless.rs", "OwnedVlessOutboundLeafConfig"),
        (
            "adapters/vless/listener.rs",
            "OwnedVlessInboundListenerConfig",
        ),
        ("adapters/vmess.rs", "OwnedVmessOutboundLeafConfig"),
        (
            "adapters/vmess/listener.rs",
            "OwnedVmessInboundListenerConfig",
        ),
        ("adapters/trojan.rs", "OwnedTrojanOutboundLeafConfig"),
        (
            "adapters/trojan/listener.rs",
            "OwnedTrojanInboundListenerConfig",
        ),
    ] {
        let source = read(&proxy_src().join(relative));
        assert!(
            !source.contains(forbidden),
            "{relative} must construct final protocol-owned leaves/requests without retaining `{forbidden}`"
        );
    }
}

#[test]
fn simpler_protocol_surfaces_do_not_expose_owned_inbound_intermediate_names() {
    for (relative, forbidden) in [
        (
            "protocols/shadowsocks/src/transport.rs",
            "OwnedShadowsocksInbound",
        ),
        ("protocols/mieru/src/transport.rs", "OwnedMieruInbound"),
        (
            "protocols/hysteria2/src/transport.rs",
            "OwnedHysteria2Inbound",
        ),
        ("protocols/socks5/src/transport.rs", "OwnedSocks5Inbound"),
        ("adapters/shadowsocks/inbound.rs", "OwnedShadowsocksInbound"),
        ("adapters/mieru/inbound.rs", "OwnedMieruInbound"),
        ("adapters/hysteria2.rs", "OwnedHysteria2Inbound"),
        ("adapters/socks5/inbound.rs", "OwnedSocks5Inbound"),
        ("adapters/mixed/inbound.rs", "OwnedSocks5Inbound"),
    ] {
        let source = if relative.starts_with("protocols/") {
            read(&workspace_root().join(relative))
        } else {
            read(&proxy_src().join(relative))
        };
        assert!(
            !source.contains(forbidden),
            "{relative} must not expose transitional inbound surface `{forbidden}`"
        );
    }
}

#[test]
fn vless_public_boundary_profiles_do_not_use_owned_prefixes() {
    for (relative, forbidden) in [
        (
            "protocols/vless/src/transport.rs",
            "OwnedVlessInboundBindPlan",
        ),
        (
            "protocols/vless/src/transport.rs",
            "OwnedVlessQuicBindProfile",
        ),
        (
            "protocols/vless/src/transport.rs",
            "OwnedVlessQuicClientProfile",
        ),
        (
            "protocols/vless/src/transport.rs",
            "OwnedVlessRealityClientProfile",
        ),
        ("adapters/vless.rs", "OwnedVlessInboundBindPlan"),
        ("adapters/vless.rs", "OwnedVlessQuicBindProfile"),
        ("adapters/vless.rs", "OwnedVlessQuicClientProfile"),
        ("adapters/vless.rs", "OwnedVlessRealityClientProfile"),
    ] {
        let source = if relative.starts_with("protocols/") {
            read(&workspace_root().join(relative))
        } else {
            read(&proxy_src().join(relative))
        };
        assert!(
            !source.contains(forbidden),
            "{relative} must not expose stable VLESS boundary type `{forbidden}`"
        );
    }
}

#[test]
fn inventory_udp_dispatch_keeps_relay_choreography_outside_candidate_root() {
    let dispatch = read(&proxy_src().join("inventory/udp/dispatch.rs"));
    assert!(!dispatch.contains("ClaimedResolvedOutbound"));
    assert!(!dispatch.contains("claim_udp_outbound(resolved)?"));
    assert!(dispatch.contains("match resolved"));
    assert!(dispatch.contains("prepare_udp_outbound("));
    assert!(!dispatch.contains("dispatch_tcp_relay_prefix"));
    assert!(!dispatch.contains("prepare_udp_packet_path_pair"));
    assert!(!dispatch.contains("prepare_udp_leaf_candidate("));
    assert!(!dispatch.contains("prepare_udp_relay_chain("));
    assert!(!dispatch.contains("UdpPacketRef"));
    assert!(!dispatch.contains("ResolvedLeafOutbound"));
    assert!(!dispatch.contains("enum UdpCandidate"));
    assert!(!dispatch.contains(".outbound_leaf_runtime("));
    assert!(!dispatch.contains("start_udp_leaf_flow("));
    assert!(!dispatch.contains("start_udp_relay_chain("));
}

#[test]
fn inventory_tcp_dispatch_claims_resolved_outbound_before_prepare() {
    let dispatch = read(&proxy_src().join("inventory/tcp/dispatch.rs"));
    assert!(!dispatch.contains("ClaimedResolvedOutbound"));
    assert!(!dispatch.contains("claim_tcp_outbound(resolved)?"));
    assert!(dispatch.contains("match resolved"));
    assert!(dispatch.contains("prepare_tcp_outbound("));
    assert!(!dispatch.contains("prepare_tcp_candidate("));
    assert!(!dispatch.contains("prepare_tcp_relay_chain("));
    assert!(!dispatch.contains("ResolvedLeafOutbound"));
    assert!(!dispatch.contains(".outbound_leaf_runtime("));
}

#[test]
fn inventory_udp_dispatch_uses_adapter_context_instead_of_proxy() {
    let dispatch = read(&proxy_src().join("inventory/udp/dispatch.rs"));
    assert!(!dispatch.contains("use crate::runtime::Proxy"));
    assert!(!dispatch.contains("&Proxy"));
}

#[test]
fn inventory_udp_leaf_and_relay_use_adapter_context_instead_of_proxy() {
    for relative in ["inventory/udp/leaf.rs", "inventory/udp/relay.rs"] {
        let source = read(&proxy_src().join(relative));
        assert!(
            !source.contains("use crate::runtime::Proxy"),
            "{relative} must not import Proxy directly"
        );
        assert!(
            !source.contains("&Proxy"),
            "{relative} must not carry raw Proxy references"
        );
    }
    let udp_leaf = read(&proxy_src().join("inventory/udp/leaf.rs"));
    let udp_relay = read(&proxy_src().join("inventory/udp/relay.rs"));
    assert!(!udp_leaf.contains("pub(crate) fn prepare_udp_leaf_candidate<'a>("));
    assert!(!udp_leaf.contains("pub(crate) async fn start_udp_leaf_flow("));
    assert!(!udp_leaf.contains("find_udp_flow_leaf("));
    assert!(!udp_relay.contains("find_udp_flow_leaf("));
    assert!(!udp_relay.contains("find_udp_packet_path_leaf("));
    assert!(!udp_relay.contains("pub(crate) fn prepare_udp_packet_path_pair<'a>("));
    assert!(!udp_relay.contains("pub(crate) fn udp_relay_needs_two_streams("));
    assert!(!udp_relay.contains("pub(crate) async fn start_udp_relay_two_stream("));
    assert!(!udp_relay.contains("pub(crate) async fn start_udp_relay_final_hop("));
    assert!(!udp_leaf.contains("UdpFlowCapability"));
    assert!(!udp_relay.contains("UdpFlowCapability"));
    assert!(!udp_relay.contains("UdpPacketPathCapability"));
}

#[test]
fn inventory_udp_packet_path_builder_stays_prepared_operation_scoped() {
    let packet_path = read(&proxy_src().join("inventory/udp/packet_path.rs"));
    assert!(!packet_path.contains("ResolvedLeafOutbound"));
    assert!(!packet_path.contains("UdpPacketPathCapability"));
    assert!(packet_path.contains("PreparedUdpPacketPathOperation"));
}

#[test]
fn inventory_tcp_relay_executes_final_hop_without_old_helper_roundtrip() {
    let relay = read(&proxy_src().join("inventory/tcp/relay.rs"));
    assert!(
        !relay.contains("apply_tcp_relay_hop("),
        "tcp relay execution should keep the final prepared hop local instead of re-routing through an old helper"
    );
    assert!(
        !relay.contains("Result<(RelayCarrier, ResolvedLeafOutbound"),
        "tcp relay prefix helpers must not return raw engine leaf values once the final hop is prepared"
    );
    assert!(relay.contains("current_prepared"));
    assert!(relay.contains("stage: \"relay_last\""));
}

#[test]
fn inventory_udp_relay_executes_final_hop_without_start_helper_roundtrip() {
    let relay = read(&proxy_src().join("inventory/udp/relay.rs"));
    assert!(
        !relay.contains("self.start_udp_relay_final_hop("),
        "udp relay chain should prepare and execute the final hop locally instead of bouncing through a second start helper"
    );
    assert!(
        !relay.contains("claim_owned_outbound_leaf("),
        "udp relay chain should reuse the already claimed final hop instead of reclaiming a raw leaf"
    );
    assert!(
        !relay.contains("PreparedUdpRelayChain::FinalHop"),
        "udp relay chain should collapse the relay prefix into an opaque prepared operation before execution"
    );
    assert!(relay.contains("PreparedUdpRelayChain"));
    assert!(relay.contains("prepare_claimed_udp_relay_chain<'a>("));
    assert!(relay.contains("prepare_udp_relay_final_hop_operation"));
}

#[test]
fn inventory_relay_modules_only_prepare_claimed_chains() {
    for relative in ["inventory/tcp/relay.rs", "inventory/udp/relay.rs"] {
        let relay = read(&proxy_src().join(relative));
        assert!(
            !relay.contains("ResolvedLeafOutbound"),
            "{relative} should not carry raw engine leaf types after the inventory claim boundary"
        );
        assert!(
            !relay.contains("claim_outbound_leaf("),
            "{relative} should prepare already claimed leaves instead of reclaiming raw outbounds"
        );
    }
}

#[test]
fn udp_two_stream_transport_bridge_uses_carrier_only_relay_prefix() {
    let relay = read(&proxy_src().join("inventory/udp/relay.rs"));
    let udp = read(&proxy_src().join("protocol_registry/transport_leaf/udp.rs"));
    assert!(relay.contains("prepare_claimed_tcp_relay_chain("));
    assert!(relay.contains("dispatch_prepared_tcp_relay_carrier(post_prepared)"));
    assert!(relay.contains("dispatch_prepared_tcp_relay_carrier(get_prepared)"));
    assert!(relay.contains("prepare_owned_udp_relay_two_stream"));
    assert!(!relay.contains("prepare_tcp_relay_chain(&chain)"));
    assert!(!udp.contains("prepare_claimed_tcp_relay_chain("));
    assert!(!udp.contains("dispatch_prepared_tcp_relay_carrier(post_prepared)"));
    assert!(!udp.contains("dispatch_prepared_tcp_relay_carrier(get_prepared)"));
}

#[test]
fn managed_stream_connectors_use_runtime_services_instead_of_proxy() {
    assert_sources_exclude(
        &proxy_src().join("runtime/udp_flow/managed/stream_manager/connector"),
        &["use crate::runtime::Proxy", "&Proxy", "Option<&Proxy>"],
    );
}

#[test]
fn managed_datagram_connectors_use_runtime_services_instead_of_proxy() {
    assert_sources_exclude(
        &proxy_src().join("runtime/udp_flow/managed/datagram_manager/connector"),
        &["use crate::runtime::Proxy", "&Proxy", "Option<&Proxy>"],
    );
}

#[test]
fn registered_upstream_runtime_uses_runtime_services_instead_of_proxy() {
    assert_sources_exclude(
        &proxy_src().join("runtime/udp_flow/registered/upstream/runtime/association"),
        &["use crate::runtime::Proxy", "&Proxy"],
    );
}

#[test]
fn managed_stream_bridge_requests_use_runtime_services_instead_of_proxy() {
    assert_sources_exclude(
        &proxy_src().join("runtime/udp_flow/managed/bridge/stream_packet"),
        &[
            "use crate::runtime::Proxy",
            "Option<&'a Proxy>",
            "Option<&Proxy>",
        ],
    );
}

#[test]
fn managed_udp_forward_paths_use_runtime_services_instead_of_proxy() {
    for relative in [
        "runtime/udp_flow/managed/state/forward.rs",
        "runtime/udp_flow/managed/stream/forward.rs",
        "runtime/udp_flow/registered/forward.rs",
    ] {
        let source = read(&proxy_src().join(relative));
        assert!(
            !source.contains("use crate::runtime::Proxy"),
            "{relative} must not import Proxy directly"
        );
        assert!(
            !source.contains("&Proxy"),
            "{relative} must not carry raw Proxy references"
        );
    }
}

#[test]
fn tcp_dispatch_operations_use_runtime_services_for_connect_flows() {
    let operation = read(&proxy_src().join("runtime/tcp_dispatch/operation.rs"));
    assert!(operation.contains("TcpRuntimeServices"));
    assert!(!operation.contains("ctx.proxy()"));
}

#[test]
fn udp_dispatch_operations_use_runtime_services_for_direct_flows() {
    let operation = read(&proxy_src().join("runtime/udp_dispatch/operation.rs"));
    assert!(operation.contains("ctx.runtime_services()"));
    assert!(!operation.contains("ctx.proxy()"));
}

#[test]
fn transport_leaf_udp_execution_uses_runtime_services_instead_of_proxy() {
    let udp = read(&proxy_src().join("protocol_registry/transport_leaf/udp.rs"));
    assert!(!udp.contains("use crate::runtime::Proxy"));
    assert!(!udp.contains("ctx.proxy()"));
    assert!(!udp.contains("&Proxy"));
}

#[test]
fn packet_path_start_surfaces_use_adapter_context_instead_of_proxy() {
    for relative in [
        "runtime/udp_dispatch/packet_path.rs",
        "runtime/udp_flow/packet_path_chain.rs",
        "runtime/udp_flow/packet_path_chain/entry.rs",
    ] {
        let source = read(&proxy_src().join(relative));
        assert!(
            !source.contains("use crate::runtime::Proxy"),
            "{relative} must not import Proxy directly"
        );
    }
}

#[test]
fn udp_socket_helpers_use_runtime_services_instead_of_proxy() {
    for relative in [
        "runtime/udp_socket.rs",
        "runtime/udp_flow/packet_path_chain/carriers/udp_socket_carrier.rs",
    ] {
        let source = read(&proxy_src().join(relative));
        assert!(
            !source.contains("use crate::runtime::Proxy"),
            "{relative} must not import Proxy directly"
        );
        assert!(
            !source.contains("&Proxy"),
            "{relative} must not carry raw Proxy references"
        );
    }

    let carrier = read(
        &proxy_src().join("runtime/udp_flow/packet_path_chain/carriers/udp_socket_carrier.rs"),
    );
    assert!(carrier.contains("UdpRuntimeServices"));
}

#[test]
fn udp_delivery_helpers_use_runtime_services_instead_of_proxy() {
    let helpers = read(&proxy_src().join("runtime/udp_delivery/helpers.rs"));
    assert!(helpers.contains("UdpRuntimeServices"));
    assert!(!helpers.contains("use crate::runtime::Proxy"));
    assert!(!helpers.contains("&Proxy"));
}

#[test]
fn udp_ingress_runtime_collapses_proxy_and_services_for_session_loops() {
    let ingress = read(&proxy_src().join("runtime/udp_ingress.rs"));
    assert!(ingress.contains("struct UdpIngressRuntime"));
    assert!(ingress.contains("services: UdpRuntimeServices"));

    let association = read(&proxy_src().join("runtime/udp_association/contract.rs"));
    assert!(association.contains("struct UdpAssociationDatagramRequest"));
    assert!(association.contains("runtime: &'a UdpIngressRuntime"));
    assert!(!association.contains("use crate::runtime::Proxy"));
    assert!(!association.contains("proxy: &Proxy"));
    assert!(!association.contains("services: &UdpRuntimeServices"));

    for relative in [
        "runtime/udp_association/lifecycle.rs",
        "runtime/datagram_udp/lifecycle.rs",
        "runtime/packet_session_udp/lifecycle.rs",
        "runtime/stream_udp.rs",
        "runtime/mux_udp.rs",
    ] {
        let source = read(&proxy_src().join(relative));
        assert!(
            source.contains("UdpIngressRuntime"),
            "{relative} must route inbound UDP through the shared ingress runtime context"
        );
        assert!(
            !source.contains("UdpRuntimeServices::from_proxy(proxy)"),
            "{relative} must not rebuild services inline from raw Proxy"
        );
        assert!(
            !source.contains("dispatch_inbound_udp_packet(proxy"),
            "{relative} must not call the raw inbound UDP helper directly"
        );
    }

    for relative in ["runtime/mux_session.rs", "runtime/mux_tcp.rs"] {
        let source = read(&proxy_src().join(relative));
        assert!(
            source.contains("MuxSubstreamRuntime"),
            "{relative} must route mux sub-stream tasks through the shared mux runtime context"
        );
        assert!(
            !source.contains("use crate::runtime::Proxy"),
            "{relative} must not import Proxy directly for mux sub-stream orchestration"
        );
    }

    for relative in [
        "runtime/datagram_udp/lifecycle.rs",
        "runtime/packet_session_udp/lifecycle.rs",
        "runtime/stream_udp.rs",
        "runtime/mux_udp.rs",
        "runtime/mux_session.rs",
        "runtime/mux_tcp.rs",
    ] {
        let source = read(&proxy_src().join(relative));
        assert!(
            !source.contains("&Proxy"),
            "{relative} must not borrow raw Proxy references for outer UDP session loops"
        );
    }

    let route_runtime = read(&proxy_src().join("runtime/route_runtime.rs"));
    assert!(route_runtime.contains("struct InboundRouteRuntime"));
    assert!(route_runtime.contains("struct MuxSubstreamRuntime"));

    for relative in [
        "runtime/inbound_route/stream/dispatch.rs",
        "runtime/inbound_route/mux/dispatch.rs",
        "runtime/mux_session.rs",
    ] {
        let source = read(&proxy_src().join(relative));
        assert!(
            !source.contains("FnOnce(Proxy"),
            "{relative} must not expose raw Proxy in generic route or mux closure contracts"
        );
        assert!(
            !source.contains("FnMut(Proxy"),
            "{relative} must not expose raw Proxy in generic route or mux closure contracts"
        );
    }

    for relative in [
        "runtime/udp_dispatch/dispatch.rs",
        "runtime/udp_dispatch/forward.rs",
        "runtime/udp_dispatch/managed/forward.rs",
    ] {
        let source = read(&proxy_src().join(relative));
        assert!(
            !source.contains("use crate::runtime::Proxy"),
            "{relative} must not import Proxy directly"
        );
        assert!(
            !source.contains("&Proxy"),
            "{relative} must not carry raw Proxy references"
        );
        assert!(
            !source.contains("UdpRuntimeServices::from_proxy(proxy)"),
            "{relative} must not rebuild services inline from raw Proxy"
        );
    }

    let pipe = read(&proxy_src().join("runtime/pipe.rs"));
    assert!(pipe.contains("pub(crate) struct UdpPipe<'a> {\n    dispatch: &'a mut UdpDispatch,"));
    assert!(pipe.contains("pub(crate) fn new(dispatch: &'a mut UdpDispatch)"));
    assert!(pipe.contains("Self { dispatch }"));
    assert!(!pipe.contains("UdpDispatch::dispatch(self.dispatch, self.proxy, input)"));
    assert!(pipe.contains("UdpDispatch::dispatch(self.dispatch, input)"));
}

#[test]
fn inventory_tcp_relay_root_is_not_a_proxy_impl_bucket() {
    let relay = read(&proxy_src().join("inventory/tcp/relay.rs"));
    assert!(!relay.contains("impl Proxy"));
    assert!(!relay.contains("use crate::runtime::Proxy"));
    assert!(!relay.contains("&Proxy"));
}

#[test]
fn inventory_tcp_leaf_stays_adapter_facing() {
    let leaf = read(&proxy_src().join("inventory/tcp/leaf.rs"));
    assert!(!leaf.contains("impl Proxy"));
    assert!(!leaf.contains("use crate::runtime::Proxy"));
    assert!(!leaf.contains("&Proxy"));
    assert!(!leaf.contains("pub(crate) fn prepare_tcp_candidate<'a>("));
    assert!(!leaf.contains("pub(crate) fn prepare_tcp_relay_hop<'a>("));
    assert!(!leaf.contains("find_outbound_leaf("));
    assert!(!leaf.contains("TcpOutboundCapability"));
}

#[test]
fn inventory_tcp_dispatch_root_is_not_a_proxy_impl_bucket() {
    let dispatch = read(&proxy_src().join("inventory/tcp/dispatch.rs"));
    assert!(!dispatch.contains("impl Proxy"));
}

#[test]
fn inventory_tcp_candidate_and_dispatch_use_runtime_services_instead_of_proxy() {
    for relative in ["inventory/tcp/candidate.rs", "inventory/tcp/dispatch.rs"] {
        let source = read(&proxy_src().join(relative));
        assert!(
            !source.contains("use crate::runtime::Proxy"),
            "{relative} must not import Proxy directly"
        );
        assert!(
            !source.contains("&Proxy"),
            "{relative} must not carry raw Proxy references"
        );
        assert!(
            source.contains("TcpRuntimeServices"),
            "{relative} must use TcpRuntimeServices"
        );
    }
    let candidate = read(&proxy_src().join("inventory/tcp/candidate.rs"));
    assert!(!candidate.contains("ResolvedLeafOutbound"));
    assert!(candidate.contains("PreparedTcpCandidate"));
}

#[test]
fn inventory_runtime_delegates_leaf_claim_logic_to_registry() {
    let runtime = read(&proxy_src().join("inventory/runtime.rs"));
    assert!(runtime.contains("struct ClaimedInventoryLeaf"));
    assert!(runtime.contains("struct ClaimedRelayChain"));
    assert!(runtime.contains("fn claim_relay_chain<'a, E, F, G>("));
    assert!(!runtime.contains("enum ClaimedResolvedOutbound"));
    assert!(!runtime.contains("fn claim_tcp_outbound"));
    assert!(!runtime.contains("fn claim_udp_outbound"));
    assert!(!runtime.contains("fn claim_tcp_relay_chain"));
    assert!(!runtime.contains("fn claim_udp_relay_chain"));
    assert!(runtime.contains("self.registry.claim_outbound_leaf(leaf)"));
    assert!(!runtime.contains("claimed_tcp_outbound_leaf"));
    assert!(!runtime.contains("claimed_udp_flow_leaf"));
    assert!(!runtime.contains("claimed_udp_packet_path_leaf"));
    assert!(!runtime.contains("find_outbound_leaf("));
    assert!(!runtime.contains("find_udp_flow_leaf("));
    assert!(!runtime.contains("find_udp_packet_path_leaf("));
    assert!(!runtime.contains("ResolvedOutbound"));
    assert!(!runtime.contains("TcpPathCategory::Block"));
}

#[test]
fn registry_outbound_claim_surface_replaces_lookup_only_helpers() {
    let outbound = read(&proxy_src().join("protocol_registry/registry/outbound.rs"));
    assert!(outbound.contains("fn claim_outbound_leaf"));
    assert!(!outbound.contains("fn claimed_tcp_outbound_leaf"));
    assert!(!outbound.contains("fn claimed_udp_flow_leaf"));
    assert!(!outbound.contains("fn claimed_udp_packet_path_leaf"));
    assert!(!outbound.contains("fn find_outbound_leaf"));
    assert!(!outbound.contains("fn find_udp_flow_leaf"));
    assert!(!outbound.contains("fn find_udp_packet_path_leaf"));
    assert!(!outbound.contains("fn outbound_leaf_runtime("));
}

#[test]
fn claimed_outbound_leaf_owns_capability_preparation() {
    let outbound = read(&proxy_src().join("protocol_registry/registry/outbound.rs"));
    let capability = read(&proxy_src().join("protocol_registry/capability.rs"));
    for method in [
        "fn prepare_tcp_connect(",
        "fn prepare_tcp_relay_hop(",
        "fn prepare_udp_flow(",
        "fn udp_relay_needs_two_streams(",
        "fn prepare_owned_udp_relay_final_hop(",
        "fn prepare_owned_udp_relay_two_stream(",
        "fn prepare_udp_packet_path(",
    ] {
        assert!(
            outbound.contains(method),
            "claimed outbound leaves should expose `{method}` so inventory stays generic after claim"
        );
    }
    assert!(!outbound.contains("pub(crate) struct ClaimedOutboundLeaf<'a> {\r\n    leaf:"));
    assert!(!outbound.contains("pub(crate) struct ClaimedOutboundLeaf<'a> {\n    leaf:"));
    assert!(!outbound.contains("fn new(\n        _leaf: ResolvedLeafOutbound<'a>,"));
    assert!(!outbound.contains("fn new(\r\n        _leaf: ResolvedLeafOutbound<'a>,"));
    assert!(outbound.contains("claim_tcp_outbound_leaf(leaf.clone())"));
    assert!(outbound.contains("claim_udp_flow_leaf(leaf.clone())"));
    assert!(outbound.contains("claim_udp_packet_path_leaf(leaf.clone())"));
    assert!(outbound.contains("fn claim_udp_hooks<'a>("));
    assert!(outbound.contains("struct ClaimedTcpHooks"));
    assert!(outbound.contains("struct ClaimedUdpHooks"));
    assert!(!outbound.contains("HookClaimedTcpLeaf"));
    assert!(!outbound.contains("HookClaimedUdpLeaf"));
    assert!(!outbound.contains("HookClaimedUdpPacketPathLeaf"));
    assert!(!outbound.contains("self.leaf"));
    assert!(outbound.contains("let runtime = claimed_tcp.runtime();"));
    assert!(!outbound.contains("udp: build_udp_hooks("));
    assert!(!capability.contains(
        "fn prepare_tcp_connect<'a>(\n        &self,\n        _leaf: ResolvedLeafOutbound<'a>,"
    ));
    assert!(!capability.contains(
        "fn prepare_udp_flow<'a>(\n        &self,\n        _leaf: ResolvedLeafOutbound<'a>,"
    ));
    assert!(!capability.contains(
        "fn prepare_udp_packet_path<'a>(\n        &self,\n        _leaf: ResolvedLeafOutbound<'a>,"
    ));
}

#[test]
fn transport_bridge_adapters_offer_claim_time_tcp_projection() {
    let helper = read(&proxy_src().join("adapters/transport_bridge.rs"));
    assert!(helper.contains("struct ClaimedTransportBridgeTcpLeaf"));
    assert!(helper.contains("claim_transport_bridge_tcp_leaf"));

    for relative in [
        "adapters/vless.rs",
        "adapters/vmess.rs",
        "adapters/trojan.rs",
    ] {
        let source = read(&proxy_src().join(relative));
        assert!(
            source.contains("fn claim_tcp_outbound_leaf<'a>("),
            "{relative} should expose a claim-time TCP leaf projection path"
        );
        assert!(
            source.contains("claim_transport_bridge_tcp_leaf("),
            "{relative} should project into the shared claimed transport-bridge helper"
        );
    }
}

#[test]
fn transport_bridge_adapters_offer_claim_time_udp_projection() {
    let helper = read(&proxy_src().join("adapters/transport_bridge.rs"));
    assert!(helper.contains("struct ClaimedTransportBridgeUdpLeaf"));
    assert!(helper.contains("claim_transport_bridge_udp_leaf"));
    assert!(helper.contains("ClaimedRelayTwoStreamTransportBridgeUdpLeaf"));
    assert!(helper.contains("claim_relay_two_stream_transport_bridge_udp_leaf"));

    for relative in [
        "adapters/vless.rs",
        "adapters/vmess.rs",
        "adapters/trojan.rs",
    ] {
        let source = read(&proxy_src().join(relative));
        assert!(
            source.contains("fn claim_udp_flow_leaf<'a>("),
            "{relative} should expose a claim-time UDP leaf projection path"
        );
    }

    let vless = read(&proxy_src().join("adapters/vless.rs"));
    assert!(vless.contains("claim_relay_two_stream_transport_bridge_udp_leaf("));

    for relative in ["adapters/vmess.rs", "adapters/trojan.rs"] {
        let source = read(&proxy_src().join(relative));
        assert!(
            source.contains("claim_transport_bridge_udp_leaf("),
            "{relative} should project into the shared claimed UDP transport-bridge helper"
        );
    }
}

#[test]
fn packet_path_adapters_offer_claim_time_projection() {
    for relative in [
        "adapters/socks5.rs",
        "adapters/shadowsocks.rs",
        "adapters/hysteria2.rs",
    ] {
        let source = read(&proxy_src().join(relative));
        assert!(
            source.contains("fn claim_udp_packet_path_leaf<'a>("),
            "{relative} should expose a claim-time packet-path leaf projection path"
        );
    }

    for relative in [
        "adapters/socks5/udp.rs",
        "adapters/shadowsocks/udp.rs",
        "adapters/hysteria2/udp.rs",
    ] {
        let source = read(&proxy_src().join(relative));
        assert!(
            source.contains("ClaimedUdpPacketPathLeaf"),
            "{relative} should materialize packet-path plans into claimed leaves"
        );
        assert!(
            source.contains("claim_udp_packet_path_leaf_impl"),
            "{relative} should own the packet-path claim-time projection helper"
        );
    }
}

#[test]
fn non_transport_bridge_adapters_offer_claim_time_tcp_projection() {
    for relative in [
        "adapters/direct.rs",
        "adapters/socks5.rs",
        "adapters/shadowsocks.rs",
        "adapters/hysteria2.rs",
        "adapters/mieru.rs",
    ] {
        let source = read(&proxy_src().join(relative));
        assert!(
            source.contains("fn claim_tcp_outbound_leaf<'a>("),
            "{relative} should expose a claim-time TCP leaf projection path"
        );
    }

    for relative in [
        "adapters/direct/tcp.rs",
        "adapters/socks5/tcp.rs",
        "adapters/shadowsocks/tcp.rs",
        "adapters/hysteria2/tcp.rs",
        "adapters/mieru/tcp.rs",
    ] {
        let source = read(&proxy_src().join(relative));
        assert!(
            source.contains("ClaimedTcpOutboundLeaf"),
            "{relative} should materialize TCP leaves into claimed projections"
        );
        assert!(
            source.contains("claim_tcp_outbound_leaf_impl"),
            "{relative} should own the TCP claim-time projection helper"
        );
    }
}

#[test]
fn non_transport_bridge_adapters_offer_claim_time_udp_projection() {
    for relative in [
        "adapters/direct.rs",
        "adapters/socks5.rs",
        "adapters/shadowsocks.rs",
        "adapters/hysteria2.rs",
        "adapters/mieru.rs",
    ] {
        let source = read(&proxy_src().join(relative));
        assert!(
            source.contains("fn claim_udp_flow_leaf<'a>("),
            "{relative} should expose a claim-time UDP leaf projection path"
        );
    }

    for relative in [
        "adapters/direct/udp.rs",
        "adapters/socks5/udp.rs",
        "adapters/shadowsocks/udp.rs",
        "adapters/hysteria2/udp.rs",
        "adapters/mieru/udp.rs",
    ] {
        let source = read(&proxy_src().join(relative));
        assert!(
            source.contains("ClaimedUdpFlowLeaf"),
            "{relative} should materialize UDP flow plans into claimed projections"
        );
        assert!(
            source.contains("claim_udp_flow_leaf_impl"),
            "{relative} should own the UDP claim-time projection helper"
        );
    }
}

#[test]
fn urltest_probe_uses_generic_tcp_outbound_dispatch() {
    let urltest = read(&proxy_src().join("groups/urltest.rs"));
    assert!(urltest.contains("dispatch_tcp_outbound("));
    assert!(!urltest.contains("prepare_tcp_candidate("));
    assert!(!urltest.contains("ResolvedLeafOutbound"));
}
