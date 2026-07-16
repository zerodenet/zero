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
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
        .replace("\r\n", "\n")
}

fn read_module(path: &Path) -> String {
    let mut combined = String::new();
    if path.is_file() {
        combined.push_str(&read(path));
        combined.push('\n');
    }
    let module_dir = if path.extension().is_some_and(|extension| extension == "rs") {
        path.with_extension("")
    } else {
        path.to_path_buf()
    };
    if module_dir.is_dir() {
        for source in rust_sources(&module_dir) {
            combined.push_str(&read(&source));
            combined.push('\n');
        }
    }
    combined
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
            "ProtocolTransportBridgeAdapter",
            "mod transport_bridge;",
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
        let source = read_module(&path);
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
fn adapter_inbound_helpers_do_not_parse_top_level_inbound_protocol_enum() {
    for relative in [
        "adapters/vless/listener.rs",
        "adapters/vmess/listener.rs",
        "adapters/trojan/listener.rs",
        "adapters/hysteria2/inbound.rs",
        "adapters/socks5/inbound.rs",
        "adapters/shadowsocks/inbound.rs",
        "adapters/mieru/inbound.rs",
    ] {
        let source = read(&proxy_src().join(relative));
        for forbidden in [
            "InboundProtocolConfig",
            "match &inbound.protocol",
            "match inbound.protocol",
            "InboundConfig",
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} must receive typed protocol-owned inbound values instead of `{forbidden}`"
            );
        }
    }
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
    let inbound_operation = read_module(&proxy_src().join("runtime/inbound_operation.rs"));
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
    assert!(!capability.contains("fn claim_tcp_outbound_leaf"));
    assert!(!capability.contains("fn claim_udp_flow_leaf"));
    assert!(!capability.contains("fn claim_udp_packet_path_leaf"));
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
    let tcp_operation = read_module(&proxy_src().join("runtime/tcp_dispatch/operation.rs"));
    let udp_operation = read_module(&proxy_src().join("runtime/udp_dispatch/operation.rs"));
    assert!(tcp_leaf.contains("OutboundAdapterContext"));
    assert!(tcp_operation.contains("TcpRuntimeServices"));
    assert!(udp_operation.contains("UdpAdapterContext"));
}

#[test]
fn transport_leaf_operations_are_generic() {
    let tcp = read_module(&proxy_src().join("runtime/tcp_dispatch/operation.rs"));
    let udp = rust_sources(&proxy_src().join("protocol_registry/transport_leaf"))
        .into_iter()
        .map(|path| read(&path))
        .collect::<String>();
    assert!(tcp.contains("TransportLeafTcpConnectOperation"));
    assert!(tcp.contains("TransportLeafTcpRelayOperation"));
    assert!(tcp.contains("PreparedTransportLeaf"));
    assert!(udp.contains("prepare_transport_udp_direct"));
    assert!(udp.contains("prepare_owned_transport_udp_relay_two_stream"));
}

#[test]
fn transport_bridge_helpers_drop_raw_leaf_resolution_paths() {
    let tcp = read(&proxy_src().join("protocol_registry/transport_leaf/tcp.rs"));
    let udp = read(&proxy_src().join("protocol_registry/transport_leaf/udp.rs"));
    let combined = format!("{tcp}\n{udp}");
    assert!(!combined.contains("prepare_last_transport_bridge_leaf"));
    assert!(!combined.contains("prepare_transport_bridge_leaf"));
    assert!(!combined.contains("trait ProtocolTransportLeafResolver {"));
    assert!(!combined.contains("trait ProtocolTransportLeafResolver<'a>"));
    assert!(!combined.contains("ResolvedLeafOutbound"));
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
    assert!(!context.contains("use crate::runtime::Proxy"));
    assert!(!context.contains("from_proxy("));
    assert!(!context.contains("proxy: Proxy"));
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
fn transport_bridge_adapters_hold_protocol_runtime_handles_instead_of_protocol_pools() {
    for (relative, runtime_handle, forbidden) in [
        (
            "adapters/vless.rs",
            "VlessTransportRuntime",
            "mux_pool::MuxConnectionPool",
        ),
        (
            "adapters/vmess.rs",
            "VmessTransportRuntime",
            "VmessMuxConnectionPool",
        ),
    ] {
        let source = read(&proxy_src().join(relative));
        assert!(
            source.contains(runtime_handle),
            "{relative} should hold protocol-owned runtime handle `{runtime_handle}`"
        );
        assert!(
            !source.contains(forbidden),
            "{relative} must not retain protocol-owned pool type `{forbidden}`"
        );
        assert!(
            !source.contains(".evict_all()"),
            "{relative} must delegate reload eviction through its protocol runtime handle"
        );
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
            "VlessInboundListenerRequest::from_profile_refs",
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
fn protocol_transport_roots_do_not_reexport_legacy_inbound_helper_functions() {
    for (relative, forbidden) in [
        (
            "protocols/hysteria2/src/transport.rs",
            "inbound_profile_from_options",
        ),
        (
            "protocols/hysteria2/src/transport.rs",
            "inbound_profile_from_password",
        ),
        (
            "protocols/hysteria2/src/transport.rs",
            "inbound_tcp_acceptor",
        ),
        (
            "protocols/hysteria2/src/transport.rs",
            "accept_and_dispatch_authenticated_hysteria2_quic_session",
        ),
        (
            "protocols/mieru/src/transport.rs",
            "inbound_listener_request_from_users",
        ),
        (
            "protocols/shadowsocks/src/transport.rs",
            "inbound_listener_parts_from_options",
        ),
        (
            "protocols/shadowsocks/src/transport.rs",
            "inbound_listener_parts_from_cipher_password",
        ),
        (
            "protocols/socks5/src/transport.rs",
            "inbound_acceptor_from_users",
        ),
    ] {
        let source = read(&workspace_root().join(relative));
        assert!(
            !source.contains(forbidden),
            "{relative} must expose typed inbound surfaces instead of legacy helper `{forbidden}`"
        );
    }
}

#[test]
fn protocol_transport_roots_do_not_reexport_legacy_udp_config_helpers() {
    for (relative, forbidden) in [
        (
            "protocols/hysteria2/src/transport.rs",
            "udp_flow_resume_from_config",
        ),
        (
            "protocols/hysteria2/src/transport.rs",
            "udp_packet_path_carrier_descriptor_from_config",
        ),
        (
            "protocols/hysteria2/src/transport.rs",
            "udp_packet_path_carrier_build_from_config",
        ),
        (
            "protocols/mieru/src/transport.rs",
            "udp_flow_resume_from_config",
        ),
        (
            "protocols/shadowsocks/src/transport.rs",
            "udp_flow_resume_from_config",
        ),
        (
            "protocols/shadowsocks/src/transport.rs",
            "udp_packet_path_carrier_descriptor_from_config",
        ),
        (
            "protocols/shadowsocks/src/transport.rs",
            "udp_packet_path_carrier_codec_from_config",
        ),
        (
            "protocols/shadowsocks/src/transport.rs",
            "udp_packet_path_datagram_source_build_from_config",
        ),
        (
            "protocols/socks5/src/transport.rs",
            "udp_association_target_from_config",
        ),
        (
            "protocols/socks5/src/transport.rs",
            "udp_packet_path_carrier_descriptor_from_config",
        ),
        (
            "protocols/socks5/src/transport.rs",
            "udp_packet_path_carrier_build_from_config",
        ),
    ] {
        let source = read(&workspace_root().join(relative));
        assert!(
            !source.contains(forbidden),
            "{relative} must expose typed UDP surfaces instead of legacy helper `{forbidden}`"
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
fn vless_adapter_uses_protocol_option_refs_instead_of_private_profile_constructors() {
    let adapter = read(&proxy_src().join("adapters/vless.rs"));
    for forbidden in [
        "VlessQuicBindProfile",
        "VlessQuicClientProfile",
        "VlessRealityClientProfile",
        "fn outbound_reality_profile(",
        "fn quic_client_profile(",
        "fn quic_bind_profile(",
    ] {
        assert!(
            !adapter.contains(forbidden),
            "adapters/vless.rs must not construct protocol-private VLESS profile `{forbidden}` inline"
        );
    }
    for required in [
        "VlessInboundBindPlan",
        "VlessOutboundBuildOptionsRef",
        "VlessOutboundOptionsRef",
        "VlessQuicBindOptionsRef",
        "VlessQuicClientOptionsRef",
        "VlessRealityClientOptionsRef",
        "VlessInboundBindPlan::from_options_refs",
        "VlessOutboundLeaf::from_options_refs",
    ] {
        assert!(
            adapter.contains(required),
            "adapters/vless.rs should project through protocol-owned VLESS option/runtime surface `{required}`"
        );
    }

    let transport = read(&workspace_root().join("protocols/vless/src/transport.rs"));
    for forbidden in [
        "VlessQuicBindProfile",
        "VlessQuicClientProfile",
        "VlessRealityClientProfile",
    ] {
        assert!(
            !transport.contains(forbidden),
            "protocols/vless/src/transport.rs must not re-export protocol-private VLESS profile surface `{forbidden}`"
        );
    }
}

#[test]
fn vless_inbound_projection_happens_at_adapter_boundary() {
    let adapter = read(&proxy_src().join("adapters/vless.rs"));
    for required in [
        "VlessInboundOptionsRef",
        "VlessInboundUserRef",
        "VlessRealityServerOptionsRef",
        "VlessInboundListenerRequest::from_options_refs",
    ] {
        assert!(
            adapter.contains(required),
            "adapters/vless.rs should project through protocol-owned VLESS inbound option surface `{required}`"
        );
    }

    let listener = read(&proxy_src().join("adapters/vless/listener.rs"));
    for forbidden in [
        "VlessInboundProfile::from_config_users",
        "VlessRealityServerProfile::from_config_server",
        "VlessInboundListenerRequest::from_config_refs",
        "VlessInboundListenerRequest::from_profile_refs",
        "VlessTransportRuntime",
        "VlessInboundOptionsRef",
        "VlessInboundUserRef",
        "VlessRealityServerOptionsRef",
        "VlessInboundListenerRequest::from_options_refs",
    ] {
        assert!(
            !listener.contains(forbidden),
            "adapters/vless/listener.rs must consume prepared VLESS listener requests instead of `{forbidden}`"
        );
    }
}

#[test]
fn vmess_adapter_uses_protocol_outbound_option_refs() {
    let adapter = read(&proxy_src().join("adapters/vmess.rs"));
    for required in [
        "VmessOutboundBuildOptionsRef",
        "VmessOutboundOptionsRef",
        "VmessTransportRuntime",
        "VmessOutboundLeaf::from_options_refs",
    ] {
        assert!(
            adapter.contains(required),
            "adapters/vmess.rs should project through protocol-owned VMess outbound option surface `{required}`"
        );
    }
}

#[test]
fn vmess_inbound_projection_happens_at_adapter_boundary() {
    let adapter = read(&proxy_src().join("adapters/vmess.rs"));
    for required in [
        "VmessInboundOptionsRef",
        "VmessInboundUserRef",
        "VmessInboundListenerRequest::from_options_refs",
    ] {
        assert!(
            adapter.contains(required),
            "adapters/vmess.rs should project through protocol-owned VMess inbound option surface `{required}`"
        );
    }

    let listener = read(&proxy_src().join("adapters/vmess/listener.rs"));
    for forbidden in [
        "VmessInboundProfile::from_config_users",
        "VmessInboundListenerRequest::from_config_refs",
        "VmessInboundListenerRequest::from_profile_refs",
        "VmessTransportRuntime",
        "VmessInboundOptionsRef",
        "VmessInboundUserRef",
        "VmessInboundListenerRequest::from_options_refs",
    ] {
        assert!(
            !listener.contains(forbidden),
            "adapters/vmess/listener.rs must consume prepared VMess listener requests instead of `{forbidden}`"
        );
    }
}

#[test]
fn heavy_protocol_transport_roots_expose_named_inbound_option_surfaces() {
    for (relative, required, forbidden) in [
        (
            "protocols/vless/src/transport.rs",
            "VlessInboundUserRef",
            "BorrowedVlessInboundUserConfigParts",
        ),
        (
            "protocols/vless/src/transport.rs",
            "VlessInboundOptionsRef",
            "BorrowedVlessInboundUserConfigParts",
        ),
        (
            "protocols/vmess/src/transport.rs",
            "VmessInboundUserRef",
            "BorrowedVmessInboundUserConfigParts",
        ),
        (
            "protocols/vmess/src/transport.rs",
            "VmessInboundOptionsRef",
            "BorrowedVmessInboundUserConfigParts",
        ),
    ] {
        let source = read(&workspace_root().join(relative));
        assert!(
            source.contains(required),
            "{relative} should expose named inbound option surface `{required}`"
        );
        assert!(
            !source.contains(forbidden),
            "{relative} must not re-export borrowed tuple inbound surface `{forbidden}`"
        );
    }
}

#[test]
fn heavy_protocol_inbound_modules_do_not_keep_borrowed_tuple_surfaces() {
    for (relative, forbidden) in [
        (
            "protocols/vless/src/inbound.rs",
            "BorrowedVlessInboundUserConfigParts",
        ),
        (
            "protocols/vmess/src/inbound.rs",
            "BorrowedVmessInboundUserConfigParts",
        ),
    ] {
        let source = read(&workspace_root().join(relative));
        assert!(
            !source.contains(forbidden),
            "{relative} must not keep transitional borrowed tuple surface `{forbidden}`"
        );
    }
}

#[test]
fn heavy_protocol_transport_roots_expose_named_outbound_build_surfaces() {
    for (relative, required, forbidden) in [
        (
            "protocols/vless/src/transport.rs",
            "VlessOutboundBuildOptionsRef",
            None,
        ),
        (
            "protocols/vmess/src/transport.rs",
            "VmessOutboundBuildOptionsRef",
            None,
        ),
        (
            "protocols/trojan/src/transport.rs",
            "TrojanOutboundBuildOptionsRef",
            Some("TrojanTransportRuntime"),
        ),
    ] {
        let source = read(&workspace_root().join(relative));
        assert!(
            source.contains(required),
            "{relative} should expose named outbound build surface `{required}`"
        );
        if let Some(forbidden) = forbidden {
            assert!(
                !source.contains(forbidden),
                "{relative} must not re-export transitional runtime wrapper `{forbidden}`"
            );
        }
    }
}

#[test]
fn trojan_adapter_uses_protocol_option_refs_directly() {
    let adapter = read(&proxy_src().join("adapters/trojan.rs"));
    for forbidden in [
        "TrojanOutboundLeaf::from_config_refs",
        "TrojanTransportRuntime",
    ] {
        assert!(
            !adapter.contains(forbidden),
            "adapters/trojan.rs must not rely on legacy Trojan outbound surface `{forbidden}`"
        );
    }
    for required in [
        "TrojanOutboundBuildOptionsRef",
        "TrojanOutboundOptionsRef",
        "TrojanOutboundLeaf::from_options_refs",
    ] {
        assert!(
            adapter.contains(required),
            "adapters/trojan.rs should project directly through protocol-owned Trojan option surface `{required}`"
        );
    }
}

#[test]
fn carrier_rich_protocol_transport_runtimes_only_keep_reloadable_state() {
    for relative in [
        "protocols/vless/src/transport/runtime.rs",
        "protocols/vmess/src/transport/runtime.rs",
    ] {
        let source = read(&workspace_root().join(relative));
        assert!(
            source.contains("evict_all()"),
            "{relative} should keep reload-driven pool eviction state"
        );
        for forbidden in [
            "build_outbound_leaf(",
            "from_profile_refs(",
            "from_options_refs(",
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} must not keep constructor passthrough `{forbidden}`"
            );
        }
    }
}

#[test]
fn heavy_protocol_transport_files_do_not_expose_public_config_constructors() {
    for (relative, required) in [
        (
            "protocols/vless/src/transport/inbound.rs",
            "pub(in crate::transport) fn from_profile_refs",
        ),
        (
            "protocols/vless/src/transport/leaf.rs",
            "pub(in crate::transport) fn from_profile_refs",
        ),
        (
            "protocols/vmess/src/transport/inbound.rs",
            "pub(in crate::transport) fn from_profile_refs",
        ),
        (
            "protocols/vmess/src/transport/leaf.rs",
            "pub(in crate::transport) fn from_profile_refs",
        ),
        (
            "protocols/trojan/src/transport/inbound.rs",
            "pub fn from_options_refs",
        ),
        (
            "protocols/trojan/src/transport/leaf.rs",
            "pub fn from_options_refs",
        ),
    ] {
        let source = read(&workspace_root().join(relative));
        assert!(
            source.contains(required),
            "{relative} should expose narrowed transport-owned constructor `{required}`"
        );
        assert!(
            !source.contains("pub fn from_config_refs"),
            "{relative} must not expose legacy public config constructor `pub fn from_config_refs`"
        );
    }
}

#[test]
fn carrier_rich_outbound_transport_leaves_expose_public_option_ref_constructors() {
    for (relative, required) in [
        (
            "protocols/vless/src/transport/leaf.rs",
            "pub fn from_options_refs",
        ),
        (
            "protocols/vmess/src/transport/leaf.rs",
            "pub fn from_options_refs",
        ),
        (
            "protocols/trojan/src/transport/leaf.rs",
            "pub fn from_options_refs",
        ),
    ] {
        let source = read(&workspace_root().join(relative));
        assert!(
            source.contains(required),
            "{relative} should expose typed public outbound option constructor `{required}`"
        );
    }
}

#[test]
fn heavy_protocol_inbound_transport_requests_expose_public_option_ref_constructors() {
    for (relative, required) in [
        (
            "protocols/vless/src/transport/inbound.rs",
            "pub fn from_options_refs",
        ),
        (
            "protocols/vmess/src/transport/inbound.rs",
            "pub fn from_options_refs",
        ),
        (
            "protocols/trojan/src/transport/inbound.rs",
            "pub fn from_options_refs",
        ),
    ] {
        let source = read(&workspace_root().join(relative));
        assert!(
            source.contains(required),
            "{relative} should expose typed public option constructor `{required}`"
        );
    }
}

#[test]
fn hysteria2_adapter_uses_protocol_bind_option_refs() {
    let adapter = read(&proxy_src().join("adapters/hysteria2.rs"));
    let forbidden = "Hysteria2InboundBindPlan::from_paths";
    assert!(
        !adapter.contains(forbidden),
        "adapters/hysteria2.rs must not build hysteria2 bind plans via `{forbidden}`"
    );
    for required in [
        "Hysteria2InboundBindOptionsRef",
        "Hysteria2InboundBindPlan::from_options_refs",
    ] {
        assert!(
            adapter.contains(required),
            "adapters/hysteria2.rs should project through protocol-owned hysteria2 bind option surface `{required}`"
        );
    }
}

#[test]
fn hysteria2_adapter_uses_protocol_outbound_option_refs() {
    let adapter = read(&proxy_src().join("adapters/hysteria2.rs"));
    let forbidden = "Hysteria2TransportLeaf::new(";
    assert!(
        !adapter.contains(forbidden),
        "adapters/hysteria2.rs must not construct hysteria2 transport leaves via `{forbidden}`"
    );
    for required in [
        "Hysteria2OutboundOptionsRef",
        "Hysteria2TransportLeaf::from_options_refs",
    ] {
        assert!(
            adapter.contains(required),
            "adapters/hysteria2.rs should project outbound through protocol-owned hysteria2 option surface `{required}`"
        );
    }
}

#[test]
fn hysteria2_inbound_projection_happens_at_adapter_boundary() {
    let adapter = read(&proxy_src().join("adapters/hysteria2.rs"));
    for required in [
        "Hysteria2InboundOptionsRef",
        "Hysteria2AuthenticatedInboundProfile::from_options_refs",
    ] {
        assert!(
            adapter.contains(required),
            "adapters/hysteria2.rs should project through protocol-owned hysteria2 inbound option surface `{required}`"
        );
    }

    let listener = read(&proxy_src().join("adapters/hysteria2/inbound.rs"));
    let forbidden = "inbound_profile_from_options";
    assert!(
        !listener.contains(forbidden),
        "adapters/hysteria2/inbound.rs must not construct hysteria2 inbound profile via `{forbidden}`"
    );
    for forbidden in [
        "Hysteria2InboundOptionsRef",
        "Hysteria2AuthenticatedInboundProfile::from_options_refs",
    ] {
        assert!(
            !listener.contains(forbidden),
            "adapters/hysteria2/inbound.rs must consume prepared hysteria2 profiles instead of `{forbidden}`"
        );
    }
}

#[test]
fn hysteria2_protocol_inbound_surface_stops_at_authenticated_quic_connections() {
    let inbound = read(&workspace_root().join("protocols/hysteria2/src/inbound.rs"));
    for forbidden in [
        "dispatch_session_with_handlers",
        "accept_and_dispatch_authenticated_quic_session",
        "JoinSet",
        "tokio::select!",
    ] {
        assert!(
            !inbound.contains(forbidden),
            "protocols/hysteria2/src/inbound.rs must not own QUIC task orchestration via `{forbidden}`"
        );
    }
    for required in [
        "pub async fn accept_authenticated_quic_session",
        "pub async fn accept_next_tcp_stream",
        "pub fn accept_udp_session(&self)",
    ] {
        assert!(
            inbound.contains(required),
            "protocols/hysteria2/src/inbound.rs should stop at authenticated QUIC connection surfaces like `{required}`"
        );
    }
}

#[test]
fn shadowsocks_adapter_accepts_protocol_stream_before_runtime_handoff() {
    let listener = read(&proxy_src().join("adapters/shadowsocks/inbound.rs"));
    assert!(
        listener.contains(".accept_stream("),
        "adapters/shadowsocks/inbound.rs should accept a protocol-owned stream surface before runtime handoff"
    );
    assert!(
        !listener.contains("accept_and_dispatch_stream"),
        "adapters/shadowsocks/inbound.rs must not use legacy accept-and-dispatch helpers"
    );

    let inbound = read(&workspace_root().join("protocols/shadowsocks/src/inbound.rs"));
    assert!(
        !inbound.contains("accept_and_dispatch_stream"),
        "protocols/shadowsocks/src/inbound.rs must stop at accept_stream instead of owning runtime handoff helpers"
    );
}

#[test]
fn mieru_adapter_accepts_protocol_session_before_runtime_handoff() {
    let listener = read(&proxy_src().join("adapters/mieru/inbound.rs"));
    assert!(
        listener.contains(".accept_client("),
        "adapters/mieru/inbound.rs should accept a protocol-owned session surface before runtime handoff"
    );
    assert!(
        listener.contains("MieruInboundAcceptedSession::Tcp"),
        "adapters/mieru/inbound.rs should explicitly branch on accepted mieru TCP sessions"
    );
    assert!(
        listener.contains("MieruInboundAcceptedSession::Udp"),
        "adapters/mieru/inbound.rs should explicitly branch on accepted mieru UDP sessions"
    );

    let inbound = read(&workspace_root().join("protocols/mieru/src/inbound.rs"));
    assert!(
        !inbound.contains("pub async fn dispatch<"),
        "protocols/mieru/src/inbound.rs must stop at accepted session surfaces instead of owning adapter dispatch helpers"
    );
}

#[test]
fn vmess_transport_accepts_client_owned_surface_without_route_wrappers() {
    let transport = read(&workspace_root().join("protocols/vmess/src/transport/inbound.rs"));
    assert!(
        transport.contains(".accept_client_owned("),
        "protocols/vmess/src/transport/inbound.rs should consume the owned accepted client surface directly"
    );
    assert!(
        !transport.contains(".accept_route_owned("),
        "protocols/vmess/src/transport/inbound.rs must not round-trip through legacy route wrapper helpers"
    );

    let inbound = read(&workspace_root().join("protocols/vmess/src/inbound.rs"));
    assert!(
        !inbound.contains("pub async fn accept_route_owned"),
        "protocols/vmess/src/inbound.rs must not expose legacy owned route wrapper helpers"
    );
    assert!(
        !inbound.contains("pub async fn accept_route_owned_with"),
        "protocols/vmess/src/inbound.rs must not expose callback-based owned route helpers"
    );
}

#[test]
fn trojan_transport_accepts_client_owned_surface_without_route_wrappers() {
    let transport = read(&workspace_root().join("protocols/trojan/src/transport/inbound.rs"));
    assert!(
        transport.contains(".accept_client_owned("),
        "protocols/trojan/src/transport/inbound.rs should consume the owned accepted client surface directly"
    );
    assert!(
        !transport.contains(".accept_route_owned("),
        "protocols/trojan/src/transport/inbound.rs must not round-trip through legacy route wrapper helpers"
    );

    let inbound = read(&workspace_root().join("protocols/trojan/src/inbound.rs"));
    assert!(
        !inbound.contains("pub async fn accept_route_owned"),
        "protocols/trojan/src/inbound.rs must not expose legacy owned route wrapper helpers"
    );
    assert!(
        !inbound.contains("pub async fn accept_route_owned_with"),
        "protocols/trojan/src/inbound.rs must not expose callback-based owned route helpers"
    );
}

#[test]
fn vless_transport_plan_accepts_client_owned_surface_without_callback_wrappers() {
    let plan = read(&workspace_root().join("protocols/vless/src/transport/inbound/plan.rs"));
    assert!(
        plan.contains(".accept_client_owned("),
        "protocols/vless/src/transport/inbound/plan.rs should consume the owned accepted client surface directly"
    );
    assert!(
        !plan.contains(".accept_route_owned_with_sni_or_else("),
        "protocols/vless/src/transport/inbound/plan.rs must not round-trip through callback-based route wrapper helpers"
    );

    let inbound = read(&workspace_root().join("protocols/vless/src/inbound.rs"));
    assert!(
        !inbound.contains("pub async fn accept_route_owned_with"),
        "protocols/vless/src/inbound.rs must not expose callback-based owned route helpers"
    );
}

#[test]
fn socks5_adapter_accepts_protocol_request_before_runtime_handoff() {
    let listener = read(&proxy_src().join("adapters/socks5/inbound/listener.rs"));
    assert!(
        listener.contains(".accept_command(&mut metered)"),
        "adapters/socks5/inbound/listener.rs should accept a protocol-owned request surface before runtime handoff"
    );
    assert!(
        listener.contains("Socks5Request::Connect"),
        "adapters/socks5/inbound/listener.rs should explicitly branch on accepted SOCKS5 CONNECT requests"
    );
    assert!(
        listener.contains("Socks5Request::UdpAssociate"),
        "adapters/socks5/inbound/listener.rs should explicitly branch on accepted SOCKS5 UDP ASSOCIATE requests"
    );
    assert!(
        listener.contains("setup_inbound_udp_association(&mut metered, request)"),
        "adapters/socks5/inbound/listener.rs should keep SOCKS5 UDP associate setup explicit at the adapter/runtime boundary"
    );

    let inbound = read(&workspace_root().join("protocols/socks5/src/inbound.rs"));
    assert!(
        !inbound.contains("pub async fn dispatch_with_handlers"),
        "protocols/socks5/src/inbound.rs must stop at accepted request surfaces instead of owning adapter dispatch helpers"
    );
}

#[test]
fn protocol_mux_servers_dispatch_opened_routes_without_handler_wrappers() {
    for relative in ["protocols/vless/src/mux.rs", "protocols/vmess/src/mux.rs"] {
        let source = read(&workspace_root().join(relative));
        assert!(
            !source.contains("dispatch_with_handlers("),
            "{relative} must not keep callback-style opened-route dispatch helpers"
        );
        assert!(
            !source.contains("dispatch_next_opened_route_with_handlers("),
            "{relative} must not keep secondary handler-wrapper dispatch entrypoints"
        );
        assert!(
            source.contains("match route.state"),
            "{relative} should branch explicitly on opened mux routes"
        );
    }
}

#[test]
fn heavy_protocol_inbound_transport_requests_do_not_keep_listener_config_wrappers() {
    for relative in [
        "protocols/vless/src/transport/inbound.rs",
        "protocols/vmess/src/transport/inbound.rs",
        "protocols/trojan/src/transport/inbound.rs",
    ] {
        let source = read(&workspace_root().join(relative));
        assert!(
            !source.contains("OwnedVlessInboundListenerConfig")
                && !source.contains("OwnedVmessInboundListenerConfig")
                && !source.contains("OwnedTrojanInboundListenerConfig"),
            "{relative} must build final inbound listener requests directly instead of keeping listener-config wrapper intermediates"
        );
        assert!(
            !source.contains("impl From<Owned"),
            "{relative} must not rely on wrapper-to-request conversion shims"
        );
    }
}

#[test]
fn heavy_protocol_outbound_transport_leaves_do_not_keep_leaf_config_wrappers() {
    for relative in [
        "protocols/vless/src/transport/leaf.rs",
        "protocols/vmess/src/transport/leaf.rs",
        "protocols/trojan/src/transport/leaf.rs",
    ] {
        let source = read(&workspace_root().join(relative));
        assert!(
            !source.contains("OwnedVlessOutboundLeafConfig")
                && !source.contains("OwnedVmessOutboundLeafConfig")
                && !source.contains("OwnedTrojanOutboundLeafConfig"),
            "{relative} must construct final outbound leaves directly instead of keeping leaf-config wrapper intermediates"
        );
        assert!(
            !source.contains("impl From<Owned"),
            "{relative} must not rely on wrapper-to-leaf conversion shims"
        );
    }
}

#[test]
fn mieru_adapter_uses_protocol_outbound_option_refs() {
    let adapter = read(&proxy_src().join("adapters/mieru.rs"));
    let forbidden = "MieruTransportLeaf::new(";
    assert!(
        !adapter.contains(forbidden),
        "adapters/mieru.rs must not construct mieru transport leaves via `{forbidden}`"
    );
    for required in [
        "MieruOutboundOptionsRef",
        "MieruTransportLeaf::from_options_refs",
    ] {
        assert!(
            adapter.contains(required),
            "adapters/mieru.rs should project outbound through protocol-owned mieru option surface `{required}`"
        );
    }
}

#[test]
fn mieru_inbound_projection_happens_at_adapter_boundary() {
    let adapter = read(&proxy_src().join("adapters/mieru.rs"));
    for required in [
        "MieruInboundUserRef",
        "MieruInboundListenerRequest::from_options_refs",
    ] {
        assert!(
            adapter.contains(required),
            "adapters/mieru.rs should project through protocol-owned mieru inbound option surface `{required}`"
        );
    }

    let listener = read(&proxy_src().join("adapters/mieru/inbound.rs"));
    let forbidden = "inbound_listener_request_from_users";
    assert!(
        !listener.contains(forbidden),
        "adapters/mieru/inbound.rs must not construct mieru inbound listener state via `{forbidden}`"
    );
    for forbidden in [
        "MieruInboundUserRef",
        "MieruInboundListenerRequest::from_options_refs",
    ] {
        assert!(
            !listener.contains(forbidden),
            "adapters/mieru/inbound.rs must consume prepared mieru listener requests instead of `{forbidden}`"
        );
    }
}

#[test]
fn shadowsocks_adapter_uses_protocol_outbound_option_refs() {
    let adapter = read(&proxy_src().join("adapters/shadowsocks.rs"));
    let forbidden = "ShadowsocksTransportLeaf::new(";
    assert!(
        !adapter.contains(forbidden),
        "adapters/shadowsocks.rs must not construct shadowsocks transport leaves via `{forbidden}`"
    );
    for required in [
        "ShadowsocksOutboundOptionsRef",
        "ShadowsocksTransportLeaf::from_options_refs",
    ] {
        assert!(
            adapter.contains(required),
            "adapters/shadowsocks.rs should project outbound through protocol-owned shadowsocks option surface `{required}`"
        );
    }
}

#[test]
fn shadowsocks_inbound_projection_happens_at_adapter_boundary() {
    let adapter = read(&proxy_src().join("adapters/shadowsocks.rs"));
    for required in [
        "ShadowsocksInboundOptionsRef",
        "ShadowsocksInboundBindings::from_options_refs",
    ] {
        assert!(
            adapter.contains(required),
            "adapters/shadowsocks.rs should project through protocol-owned shadowsocks inbound option surface `{required}`"
        );
    }

    let listener = read(&proxy_src().join("adapters/shadowsocks/inbound.rs"));
    let forbidden = "inbound_listener_parts_from_options";
    assert!(
        !listener.contains(forbidden),
        "adapters/shadowsocks/inbound.rs must not construct shadowsocks inbound listener state via `{forbidden}`"
    );
    assert!(
        listener.contains("ShadowsocksInboundBindings"),
        "adapters/shadowsocks/inbound.rs should consume prepared shadowsocks bindings"
    );
    for forbidden in [
        "ShadowsocksInboundOptionsRef",
        "ShadowsocksInboundBindings::from_options_refs",
    ] {
        assert!(
            !listener.contains(forbidden),
            "adapters/shadowsocks/inbound.rs must consume prepared shadowsocks bindings instead of `{forbidden}`"
        );
    }
}

#[test]
fn socks5_adapter_uses_protocol_outbound_option_refs() {
    let adapter = read(&proxy_src().join("adapters/socks5.rs"));
    let forbidden = "Socks5TransportLeaf::new(";
    assert!(
        !adapter.contains(forbidden),
        "adapters/socks5.rs must not construct socks5 transport leaves via `{forbidden}`"
    );
    for required in [
        "Socks5OutboundOptionsRef",
        "Socks5TransportLeaf::from_options_refs",
    ] {
        assert!(
            adapter.contains(required),
            "adapters/socks5.rs should project outbound through protocol-owned socks5 option surface `{required}`"
        );
    }
}

#[test]
fn socks5_inbound_projection_happens_at_adapter_boundary() {
    let adapter = read(&proxy_src().join("adapters/socks5.rs"));
    for required in [
        "Socks5InboundUserRef",
        "Socks5InboundAcceptor::from_options_refs",
    ] {
        assert!(
            adapter.contains(required),
            "adapters/socks5.rs should project through protocol-owned socks5 inbound option surface `{required}`"
        );
    }

    let listener = read(&proxy_src().join("adapters/socks5/inbound.rs"));
    let forbidden = "inbound_acceptor_from_users";
    assert!(
        !listener.contains(forbidden),
        "adapters/socks5/inbound.rs must not construct socks5 inbound acceptors via `{forbidden}`"
    );
    assert!(
        listener.contains("Socks5InboundAcceptor"),
        "adapters/socks5/inbound.rs should consume a prepared socks5 acceptor"
    );
    for forbidden in [
        "Socks5InboundUserRef",
        "Socks5InboundAcceptor::from_options_refs",
    ] {
        assert!(
            !listener.contains(forbidden),
            "adapters/socks5/inbound.rs must consume prepared socks5 acceptors instead of `{forbidden}`"
        );
    }
}

#[test]
fn mixed_listener_adapter_uses_protocol_socks5_option_refs() {
    let listener = read(&proxy_src().join("adapters/mixed/inbound.rs"));
    let forbidden = "inbound_acceptor_from_users";
    assert!(
        !listener.contains(forbidden),
        "adapters/mixed/inbound.rs must not construct mixed socks5 acceptors via `{forbidden}`"
    );
    for required in [
        "Socks5InboundUserRef",
        "Socks5InboundAcceptor::from_options_refs",
    ] {
        assert!(
            listener.contains(required),
            "adapters/mixed/inbound.rs should project through protocol-owned socks5 option surface `{required}`"
        );
    }
}

#[test]
fn trojan_inbound_projection_happens_at_adapter_boundary() {
    let adapter = read(&proxy_src().join("adapters/trojan.rs"));
    for required in [
        "TrojanInboundOptionsRef",
        "TrojanInboundListenerRequest::from_options_refs",
    ] {
        assert!(
            adapter.contains(required),
            "adapters/trojan.rs should project through protocol-owned Trojan inbound option surface `{required}`"
        );
    }

    let listener = read(&proxy_src().join("adapters/trojan/listener.rs"));
    for forbidden in [
        "TrojanInboundProfile::from_config_password",
        "TrojanInboundListenerRequest::from_config_refs",
        "TrojanInboundListenerRequest::from_profile_refs",
        "TrojanTransportRuntime",
        "TrojanInboundOptionsRef",
        "TrojanInboundListenerRequest::from_options_refs",
    ] {
        assert!(
            !listener.contains(forbidden),
            "adapters/trojan/listener.rs must consume prepared Trojan listener requests instead of `{forbidden}`"
        );
    }
}

#[test]
fn heavy_transport_bridge_adapters_centralize_outbound_projection() {
    for (relative, helper, variant, build_options) in [
        (
            "adapters/vless.rs",
            "VlessOutboundProjection",
            "ResolvedLeafOutbound::Vless {",
            "VlessOutboundBuildOptionsRef {",
        ),
        (
            "adapters/vmess.rs",
            "VmessOutboundProjection",
            "ResolvedLeafOutbound::Vmess {",
            "VmessOutboundBuildOptionsRef {",
        ),
        (
            "adapters/trojan.rs",
            "TrojanOutboundProjection",
            "ResolvedLeafOutbound::Trojan {",
            "TrojanOutboundBuildOptionsRef {",
        ),
    ] {
        let source = read(&proxy_src().join(relative));
        assert!(
            source.contains(&format!("struct {helper}")),
            "{relative} should keep one named outbound projection helper `{helper}`"
        );
        assert!(
            source.contains("fn from_leaf("),
            "{relative} should centralize outbound leaf matching behind `from_leaf`"
        );
        assert!(
            source.contains("fn build_options("),
            "{relative} should centralize protocol-owned outbound build bundle construction"
        );
        assert!(
            source.contains("fn claim_outbound_leaf_impl"),
            "{relative} should expose one claim-time outbound helper that reuses the shared projection"
        );
        assert_eq!(
            source.matches(variant).count(),
            1,
            "{relative} should match its heavy outbound leaf variant in one projection helper"
        );
        assert_eq!(
            source.matches(build_options).count(),
            1,
            "{relative} should construct one outbound build bundle shape and reuse it across TCP and UDP"
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
    let operation = read_module(&proxy_src().join("runtime/tcp_dispatch/operation.rs"));
    assert!(operation.contains("TcpRuntimeServices"));
    assert!(!operation.contains("ctx.proxy()"));
}

#[test]
fn udp_dispatch_operations_use_runtime_services_for_direct_flows() {
    let operation = read_module(&proxy_src().join("runtime/udp_dispatch/operation.rs"));
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
    let helpers = read_module(&proxy_src().join("runtime/udp_delivery/helpers.rs"));
    assert!(helpers.contains("UdpRuntimeServices"));
    assert!(!helpers.contains("use crate::runtime::Proxy"));
    assert!(!helpers.contains("&Proxy"));
}

#[test]
fn udp_delivery_helpers_root_stays_facade_only() {
    let helpers_root = read(&proxy_src().join("runtime/udp_delivery/helpers.rs"));
    let helpers = read_module(&proxy_src().join("runtime/udp_delivery/helpers.rs"));
    for module_name in [
        "mod accounting;",
        "mod lifecycle;",
        "mod parts;",
        "mod response;",
    ] {
        assert!(helpers_root.contains(module_name));
    }
    for forbidden in [
        "pub(crate) struct UdpInboundResponseAccounting",
        "pub(crate) fn log_completed_udp_flow",
        "pub(crate) fn record_direct_udp_response_parts",
        "pub(crate) fn record_chain_udp_response_parts",
        "pub(crate) fn record_upstream_udp_response_received",
    ] {
        assert!(
            !helpers_root.contains(forbidden),
            "udp_delivery helpers facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "pub(crate) struct UdpInboundResponseAccounting",
        "pub(crate) fn log_completed_udp_flow",
        "pub(crate) fn record_direct_udp_response_parts",
        "pub(crate) fn record_chain_udp_response_parts",
        "pub(crate) fn record_upstream_udp_response_received",
    ] {
        assert!(
            helpers.contains(expected),
            "udp_delivery helpers module tree must still provide `{expected}`"
        );
    }
}

#[test]
fn udp_ingress_runtime_collapses_proxy_and_services_for_session_loops() {
    let ingress = read_module(&proxy_src().join("runtime/udp_ingress.rs"));
    assert!(ingress.contains("struct UdpIngressRuntime"));
    assert!(ingress.contains("services: UdpRuntimeServices"));
    assert!(!ingress.contains("use crate::runtime::Proxy"));
    assert!(!ingress.contains("proxy: Proxy"));
    assert!(!ingress.contains("from_proxy("));

    let association = read_module(&proxy_src().join("runtime/udp_association/contract.rs"));
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
        let source = read_module(&proxy_src().join(relative));
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
        let source = read_module(&proxy_src().join(relative));
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
        let source = read_module(&proxy_src().join(relative));
        assert!(
            !source.contains("&Proxy"),
            "{relative} must not borrow raw Proxy references for outer UDP session loops"
        );
    }

    let route_runtime_root = read(&proxy_src().join("runtime/route_runtime.rs"));
    let route_runtime = read_module(&proxy_src().join("runtime/route_runtime.rs"));
    assert!(route_runtime_root.contains("mod listener;"));
    assert!(route_runtime_root.contains("mod route;"));
    assert!(route_runtime_root.contains("mod shared;"));
    assert!(!route_runtime_root.contains("struct InboundRouteRuntime"));
    assert!(!route_runtime_root.contains("struct InboundListenerRuntime"));
    assert!(!route_runtime_root.contains("struct SharedIngressRuntimeServices"));
    assert!(route_runtime.contains("struct InboundRouteRuntime"));
    assert!(route_runtime.contains("struct InboundListenerRuntime"));
    assert!(route_runtime.contains("struct SharedIngressRuntimeServices"));
    assert!(route_runtime.contains("TcpIngressRuntime"));
    assert!(route_runtime.contains("tcp_runtime: TcpIngressRuntime"));
    assert!(!route_runtime.contains("use crate::runtime::Proxy"));
    assert!(!route_runtime.contains("from_proxy("));
    assert!(!route_runtime.contains("fallback_proxy"));
    assert!(!route_runtime.contains("proxy: Proxy"));
    #[cfg(any(feature = "vless", feature = "vmess"))]
    assert!(route_runtime.contains("struct MuxSubstreamRuntime"));

    let inbound_operation = read_module(&proxy_src().join("runtime/inbound_operation.rs"));
    assert!(inbound_operation.contains("InboundListenerRuntime"));
    assert!(!inbound_operation.contains("proxy: Proxy"));
    assert!(!inbound_operation.contains("execute(proxy"));

    let inbound_context_root = read(&proxy_src().join("runtime/inbound_operation/context.rs"));
    let inbound_context = read_module(&proxy_src().join("runtime/inbound_operation/context.rs"));
    for module_name in [
        "mod model;",
        "mod no_client;",
        "mod recorded;",
        "mod serve;",
        "mod udp;",
    ] {
        assert!(inbound_context_root.contains(module_name));
    }
    for forbidden in [
        "pub(crate) async fn run_udp_association",
        "pub(crate) async fn serve<P>(",
        "pub(crate) async fn dispatch_no_client_stream_route",
        "pub(crate) async fn dispatch_recorded_mux_tcp_route",
    ] {
        assert!(
            !inbound_context_root.contains(forbidden),
            "inbound_operation context facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "pub(crate) async fn run_udp_association",
        "pub(crate) async fn serve<P>(",
        "pub(crate) async fn dispatch_no_client_stream_route",
        "pub(crate) async fn dispatch_recorded_mux_tcp_route",
    ] {
        assert!(
            inbound_context.contains(expected),
            "inbound_operation context module tree must still provide `{expected}`"
        );
    }

    let inventory_inbound = read(&proxy_src().join("inventory/inbound.rs"));
    assert!(inventory_inbound.contains("InboundListenerRuntime"));
    assert!(!inventory_inbound.contains("use crate::runtime::Proxy"));
    assert!(!inventory_inbound.contains("execute(proxy.clone()"));

    let listener_inbound = read(&proxy_src().join("runtime/listeners/inbound.rs"));
    assert!(listener_inbound.contains("ProtocolInventory"));
    assert!(listener_inbound.contains("InboundListenerRuntimeFactory"));
    assert!(!listener_inbound.contains("use super::super::Proxy"));
    assert!(!listener_inbound.contains("proxy: &Proxy"));
    assert!(!listener_inbound.contains("proxy.config.source_dir()"));

    let tcp_ingress_runtime = read_module(&proxy_src().join("runtime/tcp_ingress/runtime.rs"));
    assert!(tcp_ingress_runtime.contains("struct TcpIngressRuntime"));
    assert!(tcp_ingress_runtime.contains("serve_inbound("));
    assert!(!tcp_ingress_runtime.contains("use crate::runtime::Proxy"));
    assert!(!tcp_ingress_runtime.contains("proxy: Proxy"));

    let tcp_ingress_lifecycle = read_module(&proxy_src().join("runtime/tcp_ingress/lifecycle.rs"));
    assert!(tcp_ingress_lifecycle.contains("TcpIngressRuntime"));
    assert!(!tcp_ingress_lifecycle.contains("use crate::runtime::Proxy"));
    assert!(!tcp_ingress_lifecycle.contains("proxy: &Proxy"));

    let tcp_ingress_contract = read_module(&proxy_src().join("runtime/tcp_ingress/contract.rs"));
    assert!(tcp_ingress_contract.contains("TcpRuntimeServices"));
    assert!(!tcp_ingress_contract.contains("use crate::runtime::Proxy"));
    assert!(!tcp_ingress_contract.contains("proxy: &Proxy"));

    for relative in [
        "runtime/listener_loop/tcp.rs",
        "runtime/listener_loop/quic.rs",
        "runtime/listener_loop/system.rs",
    ] {
        let source = read(&proxy_src().join(relative));
        assert!(
            !source.contains("use crate::runtime::Proxy"),
            "{relative} must not import Proxy directly"
        );
        assert!(
            !source.contains("proxy: &'a Proxy"),
            "{relative} must not retain raw Proxy in listener-loop request models"
        );
        assert!(
            !source.contains("Fn(Proxy"),
            "{relative} must not expose raw Proxy in listener-loop handler contracts"
        );
        assert!(
            !source.contains("FnMut(Proxy"),
            "{relative} must not expose raw Proxy in listener-loop handler contracts"
        );
    }

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

    let pipe = read_module(&proxy_src().join("runtime/pipe.rs"));
    assert!(pipe.contains("pub(crate) struct UdpPipe<'a> {\n    dispatch: &'a mut UdpDispatch,"));
    assert!(pipe.contains("pub(crate) fn new(dispatch: &'a mut UdpDispatch)"));
    assert!(pipe.contains("Self { dispatch }"));
    assert!(!pipe.contains("UdpDispatch::dispatch(self.dispatch, self.proxy, input)"));
    assert!(pipe.contains("UdpDispatch::dispatch(self.dispatch, input)"));
}

#[test]
fn pipe_root_stays_facade_only() {
    let pipe_root = read(&proxy_src().join("runtime/pipe.rs"));
    let pipe = read_module(&proxy_src().join("runtime/pipe.rs"));
    for module_name in ["mod contract;", "mod tcp;", "mod udp;"] {
        assert!(pipe_root.contains(module_name));
    }
    for forbidden in [
        "pub(crate) trait KernelPipe",
        "pub(crate) struct TcpPipe<'a>",
        "pub(crate) struct UdpPipe<'a>",
        "pub(crate) struct UdpPipeInput<'a>",
    ] {
        assert!(
            !pipe_root.contains(forbidden),
            "pipe facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "pub(crate) trait KernelPipe",
        "pub(crate) struct TcpPipe<'a>",
        "pub(crate) struct UdpPipe<'a>",
        "pub(crate) struct UdpPipeInput<'a>",
    ] {
        assert!(
            pipe.contains(expected),
            "pipe module tree must still provide `{expected}`"
        );
    }
}

#[test]
fn listener_loop_quic_root_stays_facade_only() {
    let quic_root = read(&proxy_src().join("runtime/listener_loop/quic.rs"));
    let quic = read_module(&proxy_src().join("runtime/listener_loop/quic.rs"));
    for module_name in ["mod connection;", "mod logged;", "mod stream;"] {
        assert!(quic_root.contains(module_name));
    }
    for forbidden in [
        "pub(crate) struct QuicListenerLoopRequest<H>",
        "pub(crate) struct QuicStreamListenerLoopRequest<H>",
        "pub(crate) struct LoggedQuicStreamListenerRequest<R, D>",
        "pub(crate) async fn run_quic_listener_loop<H, Fut>(",
        "pub(crate) async fn run_logged_quic_stream_listener_loop<R, D, Fut>(",
    ] {
        assert!(
            !quic_root.contains(forbidden),
            "listener_loop quic facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "pub(crate) struct QuicListenerLoopRequest<H>",
        "pub(crate) struct QuicStreamListenerLoopRequest<H>",
        "pub(crate) struct LoggedQuicStreamListenerRequest<R, D>",
        "pub(crate) async fn run_quic_listener_loop<H, Fut>(",
        "pub(crate) async fn run_logged_quic_stream_listener_loop<R, D, Fut>(",
    ] {
        assert!(
            quic.contains(expected),
            "listener_loop quic module tree must still provide `{expected}`"
        );
    }
}

#[test]
fn listener_loop_tcp_root_stays_facade_only() {
    let tcp_root = read(&proxy_src().join("runtime/listener_loop/tcp.rs"));
    let tcp = read_module(&proxy_src().join("runtime/listener_loop/tcp.rs"));
    for module_name in ["mod connection;", "mod logged;"] {
        assert!(tcp_root.contains(module_name));
    }
    for forbidden in [
        "pub(crate) struct TcpListenerLoopRequest<H>",
        "pub(crate) struct LoggedTcpSocketListenerRequest<R, D>",
        "pub(crate) async fn run_tcp_listener_loop<H, Fut>(",
        "pub(crate) async fn run_logged_tcp_socket_listener_loop<R, D, Fut>(",
    ] {
        assert!(
            !tcp_root.contains(forbidden),
            "listener_loop tcp facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "pub(crate) struct TcpListenerLoopRequest<H>",
        "pub(crate) struct LoggedTcpSocketListenerRequest<R, D>",
        "pub(crate) async fn run_tcp_listener_loop<H, Fut>(",
        "pub(crate) async fn run_logged_tcp_socket_listener_loop<R, D, Fut>(",
    ] {
        assert!(
            tcp.contains(expected),
            "listener_loop tcp module tree must still provide `{expected}`"
        );
    }
}

#[test]
fn udp_ingress_root_stays_facade_only() {
    let ingress_root = read(&proxy_src().join("runtime/udp_ingress.rs"));
    let ingress = read_module(&proxy_src().join("runtime/udp_ingress.rs"));
    for module_name in ["mod dispatch;", "mod model;", "mod route;", "mod session;"] {
        assert!(ingress_root.contains(module_name));
    }
    for forbidden in [
        "struct UdpIngressRuntime",
        "pub(crate) async fn new_dispatch",
        "pub(crate) fn route_decision",
        "pub(crate) fn prepare_udp_session",
        "pub(crate) async fn dispatch_inbound_packet",
    ] {
        assert!(
            !ingress_root.contains(forbidden),
            "udp_ingress facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "pub(crate) struct UdpIngressRuntime",
        "pub(crate) async fn new_dispatch",
        "pub(crate) fn route_decision",
        "pub(crate) fn prepare_udp_session",
        "pub(crate) async fn dispatch_inbound_packet",
    ] {
        assert!(
            ingress.contains(expected),
            "udp_ingress module tree must still provide `{expected}`"
        );
    }
}

#[test]
fn tcp_ingress_runtime_root_stays_facade_only() {
    let runtime_root = read(&proxy_src().join("runtime/tcp_ingress/runtime.rs"));
    let runtime = read_module(&proxy_src().join("runtime/tcp_ingress/runtime.rs"));
    for module_name in ["mod model;", "mod route;", "mod serve;", "mod session;"] {
        assert!(runtime_root.contains(module_name));
    }
    for forbidden in [
        "pub(crate) struct TcpIngressRuntime",
        "pub(crate) async fn serve<P>(",
        "pub(crate) fn route_decision",
        "pub(crate) fn prepare_session",
        "pub(crate) async fn open_tcp_upstream",
    ] {
        assert!(
            !runtime_root.contains(forbidden),
            "tcp_ingress runtime facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "pub(crate) struct TcpIngressRuntime",
        "pub(crate) async fn serve<P>(",
        "pub(crate) fn route_decision",
        "pub(crate) fn prepare_session",
        "pub(crate) async fn open_tcp_upstream",
    ] {
        assert!(
            runtime.contains(expected),
            "tcp_ingress runtime module tree must still provide `{expected}`"
        );
    }
}

#[test]
fn tcp_ingress_lifecycle_root_stays_facade_only() {
    let lifecycle_root = read(&proxy_src().join("runtime/tcp_ingress/lifecycle.rs"));
    let lifecycle = read_module(&proxy_src().join("runtime/tcp_ingress/lifecycle.rs"));
    for module_name in ["mod rate_limit;", "mod result;", "mod serve;"] {
        assert!(lifecycle_root.contains(module_name));
    }
    for forbidden in [
        "pub(crate) async fn serve_inbound<",
        "pub(crate) async fn serve_inbound_with_client_response",
        "pub(crate) fn apply_kernel_rate_limits_from_config",
        "tokio::time::timeout(",
        "fn finish_relay_success(",
    ] {
        assert!(
            !lifecycle_root.contains(forbidden),
            "tcp_ingress lifecycle facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "pub(crate) async fn serve_inbound<",
        "pub(crate) fn apply_kernel_rate_limits_from_config",
        "tokio::time::timeout(",
        "fn finish_relay_success(",
        "fn finish_route_or_establish_failure(",
    ] {
        assert!(
            lifecycle.contains(expected),
            "tcp_ingress lifecycle module tree must still provide `{expected}`"
        );
    }
}

#[test]
fn tcp_ingress_contract_root_stays_facade_only() {
    let contract_root = read(&proxy_src().join("runtime/tcp_ingress/contract.rs"));
    let contract = read_module(&proxy_src().join("runtime/tcp_ingress/contract.rs"));
    for module_name in [
        "mod accounting;",
        "mod client_response;",
        "mod no_response;",
        "mod protocol;",
    ] {
        assert!(contract_root.contains(module_name));
    }
    for forbidden in [
        "pub(crate) fn record_tcp_upload",
        "pub(crate) trait InboundProtocol",
        "pub(crate) struct ClientResponseInboundProtocol",
        "pub(crate) struct NoClientResponseStreamProtocol",
    ] {
        assert!(
            !contract_root.contains(forbidden),
            "tcp_ingress contract facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "fn record_tcp_upload(",
        "pub(crate) trait InboundProtocol",
        "pub(crate) struct ClientResponseInboundProtocol",
        "pub(crate) struct NoClientResponseStreamProtocol",
        "TcpRuntimeServices",
    ] {
        assert!(
            contract.contains(expected),
            "tcp_ingress contract module tree must still provide `{expected}`"
        );
    }
}

#[test]
fn orchestration_root_stays_facade_only() {
    let orchestration_root = read(&proxy_src().join("runtime/orchestration.rs"));
    let orchestration = read_module(&proxy_src().join("runtime/orchestration.rs"));
    for module_name in ["mod lifecycle;", "mod logging;", "mod state;"] {
        assert!(orchestration_root.contains(module_name));
    }
    for forbidden in [
        "pub(super) async fn run_until<",
        "pub(in crate::runtime) async fn run_until<",
        "struct OrchestrationState",
        "fn log_started(",
        "fn log_stopped(",
        "tokio::select!",
    ] {
        assert!(
            !orchestration_root.contains(forbidden),
            "orchestration facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "pub(in crate::runtime) async fn run_until<",
        "pub(super) struct OrchestrationState",
        "pub(super) fn log_started(",
        "pub(super) fn log_stopped(",
        "tokio::select! {",
    ] {
        assert!(
            orchestration.contains(expected),
            "orchestration module tree must still provide `{expected}`"
        );
    }
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
fn route_runtime_route_root_stays_facade_only() {
    let route_root = read(&proxy_src().join("runtime/route_runtime/route.rs"));
    let route = read_module(&proxy_src().join("runtime/route_runtime/route.rs"));
    for module_name in ["mod access;", "mod model;", "mod serve;"] {
        assert!(route_root.contains(module_name));
    }
    for forbidden in [
        "pub(crate) struct InboundRouteRuntime",
        "pub(crate) struct InboundRouteRuntimeFactory",
        "pub(crate) async fn serve<P>(",
        "pub(crate) fn for_connection(&self, source_addr: Option<SocketAddr>)",
    ] {
        assert!(
            !route_root.contains(forbidden),
            "route runtime route facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "pub(crate) struct InboundRouteRuntime",
        "pub(crate) struct InboundRouteRuntimeFactory",
        "pub(crate) async fn serve<P>(",
        "pub(crate) fn for_connection(&self, source_addr: Option<SocketAddr>)",
    ] {
        assert!(
            route.contains(expected),
            "route runtime route module tree must still provide `{expected}`"
        );
    }
}

#[test]
fn handle_command_root_stays_facade_only() {
    let command_root = read(&proxy_src().join("runtime/handle/command.rs"));
    let command = read_module(&proxy_src().join("runtime/handle/command.rs"));
    for module_name in [
        "mod diagnostics;",
        "mod dispatch;",
        "mod runtime;",
        "mod tun;",
    ] {
        assert!(command_root.contains(module_name));
    }
    for forbidden in [
        "impl zero_api::CommandService for ProxyHandle",
        "CommandRequest::TunStart",
        "CommandRequest::DiagnosticsProbeOutbound",
        "CommandRequest::DiagnosticsDnsCache",
        "CommandRequest::DiagnosticsFakeipLookup",
    ] {
        assert!(
            !command_root.contains(forbidden),
            "handle command facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "impl zero_api::CommandService for ProxyHandle",
        "CommandRequest::TunStart",
        "CommandRequest::DiagnosticsProbeOutbound",
        "CommandRequest::DiagnosticsDnsCache",
        "CommandRequest::DiagnosticsFakeipLookup",
    ] {
        assert!(
            command.contains(expected),
            "handle command module tree must still provide `{expected}`"
        );
    }
}

#[test]
fn udp_flow_outbound_root_stays_facade_only() {
    let outbound_root = read(&proxy_src().join("runtime/udp_flow/outbound.rs"));
    let outbound = read_module(&proxy_src().join("runtime/udp_flow/outbound.rs"));
    assert!(outbound_root.contains("mod model;"));
    assert!(outbound_root.contains("mod projection;"));
    assert!(!outbound_root.contains("pub(crate) fn tag(&self)"));
    assert!(!outbound_root.contains("fn upstream_endpoint(&self)"));
    assert!(outbound.contains("pub(crate) enum UdpFlowOutbound"));
    assert!(outbound.contains("pub(in crate::runtime::udp_flow) fn completion(&self)"));
}

#[test]
fn udp_flow_outbound_projection_root_stays_facade_only() {
    let projection_root = read(&proxy_src().join("runtime/udp_flow/outbound/projection.rs"));
    let projection = read_module(&proxy_src().join("runtime/udp_flow/outbound/projection.rs"));
    for module_name in ["mod access;", "mod completion;", "mod index;"] {
        assert!(projection_root.contains(module_name));
    }
    for forbidden in [
        "pub(crate) fn tag(&self)",
        "pub(crate) fn upstream(&self)",
        "pub(in crate::runtime::udp_flow) fn index_keys(&self)",
        "fn success_outcome(&self)",
        "pub(in crate::runtime::udp_flow) fn completion(&self)",
    ] {
        assert!(
            !projection_root.contains(forbidden),
            "udp_flow outbound projection facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "pub(crate) fn tag(&self)",
        "pub(crate) fn upstream(&self)",
        "pub(in crate::runtime::udp_flow) fn index_keys(&self)",
        "fn success_outcome(&self)",
        "pub(in crate::runtime::udp_flow) fn completion(&self)",
    ] {
        assert!(
            projection.contains(expected),
            "udp_flow outbound projection module tree must still provide `{expected}`"
        );
    }
}

#[test]
fn udp_flow_outbound_projection_access_root_stays_facade_only() {
    let access_root = read(&proxy_src().join("runtime/udp_flow/outbound/projection/access.rs"));
    let access = read_module(&proxy_src().join("runtime/udp_flow/outbound/projection/access.rs"));
    for module_name in ["mod identity;", "mod managed;", "mod upstream;"] {
        assert!(access_root.contains(module_name));
    }
    for forbidden in [
        "pub(crate) fn tag(&self) -> &str",
        "pub(crate) fn path_category(&self) -> UdpPathCategory",
        "pub(crate) fn managed_flow(&self) -> Option<ManagedUdpFlowRef>",
        "pub(crate) fn upstream(&self) -> Option<UdpFlowUpstream<'_>>",
    ] {
        assert!(
            !access_root.contains(forbidden),
            "udp_flow outbound projection access facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "pub(crate) fn tag(&self) -> &str",
        "pub(crate) fn path_category(&self) -> UdpPathCategory",
        "pub(crate) fn managed_flow(&self) -> Option<ManagedUdpFlowRef>",
        "pub(crate) fn upstream(&self) -> Option<UdpFlowUpstream<'_>>",
    ] {
        assert!(
            access.contains(expected),
            "udp_flow outbound projection access module tree must still provide `{expected}`"
        );
    }
}

#[test]
fn udp_flow_packet_path_root_stays_facade_only() {
    let packet_path_root = read(&proxy_src().join("runtime/udp_flow/packet_path.rs"));
    let packet_path = read_module(&proxy_src().join("runtime/udp_flow/packet_path.rs"));
    for module_name in [
        "mod carrier;",
        "mod context;",
        "mod datagram;",
        "mod snapshot;",
    ] {
        assert!(packet_path_root.contains(module_name));
    }
    for forbidden in [
        "pub(crate) type ChainTask",
        "pub(crate) trait PacketPathCarrier",
        "pub(crate) struct UdpDatagramSource",
        "pub(crate) struct PacketPathFlowBinding",
    ] {
        assert!(
            !packet_path_root.contains(forbidden),
            "packet_path facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "pub(crate) trait PacketPathCarrier",
        "pub(crate) struct UdpDatagramSource",
        "pub(crate) struct PacketPathFlowBinding",
    ] {
        assert!(
            packet_path.contains(expected),
            "packet_path module tree must still provide `{expected}`"
        );
    }
}

#[test]
fn udp_flow_packet_path_datagram_root_stays_facade_only() {
    let datagram_root = read(&proxy_src().join("runtime/udp_flow/packet_path/datagram.rs"));
    let datagram = read_module(&proxy_src().join("runtime/udp_flow/packet_path/datagram.rs"));
    for module_name in ["mod access;", "mod build;", "mod model;"] {
        assert!(datagram_root.contains(module_name));
    }
    for forbidden in [
        "pub(crate) struct UdpDatagramDescriptor",
        "pub(crate) struct UdpDatagramSource",
        "pub(crate) fn udp_datagram_source(",
        "pub(crate) fn target(&self) -> Address",
    ] {
        assert!(
            !datagram_root.contains(forbidden),
            "udp_flow packet_path datagram facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "pub(crate) struct UdpDatagramDescriptor",
        "pub(crate) struct UdpDatagramSource",
        "pub(crate) fn udp_datagram_source(",
        "pub(crate) fn target(&self) -> Address",
    ] {
        assert!(
            datagram.contains(expected),
            "udp_flow packet_path datagram module tree must still provide `{expected}`"
        );
    }
}

#[test]
fn udp_dispatch_forward_root_stays_facade_only() {
    let forward_root = read(&proxy_src().join("runtime/udp_dispatch/forward.rs"));
    let forward = read_module(&proxy_src().join("runtime/udp_dispatch/forward.rs"));
    for module_name in ["mod path;", "mod result;"] {
        assert!(forward_root.contains(module_name));
    }
    for forbidden in [
        "pub(in crate::runtime::udp_dispatch) async fn forward_existing(",
        "fn fail_flow_with_msg(",
        "fn record_or_fail(",
    ] {
        assert!(
            !forward_root.contains(forbidden),
            "udp_dispatch forward facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "pub(in crate::runtime::udp_dispatch) async fn forward_existing(",
        "pub(super) fn fail_flow_with_msg(",
        "pub(super) fn record_or_fail(",
    ] {
        assert!(
            forward.contains(expected),
            "udp_dispatch forward module tree must still provide `{expected}`"
        );
    }
}

#[test]
fn udp_dispatch_root_stays_facade_only() {
    let dispatch_root = read(&proxy_src().join("runtime/udp_dispatch/mod.rs"));
    let dispatch = read_module(&proxy_src().join("runtime/udp_dispatch"));
    for module_name in ["mod failure;", "mod model;"] {
        assert!(dispatch_root.contains(module_name));
    }
    for forbidden in [
        "pub(crate) struct UdpDispatch",
        "fn fail_flow(",
        "log_session_failed",
    ] {
        assert!(
            !dispatch_root.contains(forbidden),
            "udp_dispatch facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in ["pub(crate) struct UdpDispatch", "pub(super) fn fail_flow("] {
        assert!(
            dispatch.contains(expected),
            "udp_dispatch module tree must still provide `{expected}`"
        );
    }
}

#[test]
fn udp_dispatch_lifecycle_root_stays_facade_only() {
    let lifecycle_root = read(&proxy_src().join("runtime/udp_dispatch/lifecycle.rs"));
    let lifecycle = read_module(&proxy_src().join("runtime/udp_dispatch/lifecycle.rs"));
    for module_name in ["mod lookup;", "mod poll;", "mod setup;"] {
        assert!(lifecycle_root.contains(module_name));
    }
    for forbidden in [
        "pub(crate) struct UpstreamAssociationView<'a>",
        "pub(crate) struct ClosedUpstreamAssociation",
        "pub(crate) async fn new(",
        "pub(crate) fn poll_refs(",
        "pub(crate) fn finish_all(mut self)",
    ] {
        assert!(
            !lifecycle_root.contains(forbidden),
            "udp_dispatch lifecycle facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "pub(crate) struct UpstreamAssociationView<'a>",
        "pub(crate) struct ClosedUpstreamAssociation",
        "pub(crate) async fn new(",
        "pub(crate) fn poll_refs(",
        "pub(crate) fn finish_all(mut self)",
    ] {
        assert!(
            lifecycle.contains(expected),
            "udp_dispatch lifecycle module tree must still provide `{expected}`"
        );
    }
}

#[test]
fn udp_dispatch_managed_start_root_stays_facade_only() {
    let start_root = read(&proxy_src().join("runtime/udp_dispatch/managed/start.rs"));
    let start = read_module(&proxy_src().join("runtime/udp_dispatch/managed/start.rs"));
    for module_name in [
        "mod context;",
        "mod datagram;",
        "mod send;",
        "mod upstream;",
    ] {
        assert!(start_root.contains(module_name));
    }
    for forbidden in [
        "pub(crate) fn flow_start_context(&mut self)",
        "pub(in crate::runtime::udp_dispatch::managed) async fn send_managed_udp(",
        "pub(crate) async fn start_tracked_managed_datagram",
        "pub(crate) async fn start_tracked_upstream",
    ] {
        assert!(
            !start_root.contains(forbidden),
            "udp_dispatch managed start facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "pub(crate) fn flow_start_context(&mut self)",
        "pub(in crate::runtime::udp_dispatch::managed) async fn send_managed_udp(",
        "pub(crate) async fn start_tracked_managed_datagram",
        "pub(crate) async fn start_tracked_upstream",
    ] {
        assert!(
            start.contains(expected),
            "udp_dispatch managed start module tree must still provide `{expected}`"
        );
    }
}

#[test]
fn mux_session_root_stays_facade_only() {
    let mux_session_root = read(&proxy_src().join("runtime/mux_session.rs"));
    let mux_session = read_module(&proxy_src().join("runtime/mux_session.rs"));
    for module_name in ["mod lifecycle;", "mod model;", "mod protocol;"] {
        assert!(mux_session_root.contains(module_name));
    }
    for forbidden in [
        "pub(crate) struct MuxSessionLoop",
        "pub(crate) trait MuxOpenedDispatcher",
        "pub(crate) async fn run_mux_session_loop",
        "pub(crate) fn drain_completed_mux_tasks",
        "pub(crate) async fn run_protocol_mux_session",
    ] {
        assert!(
            !mux_session_root.contains(forbidden),
            "mux_session facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "pub(crate) struct MuxSessionLoop",
        "pub(crate) trait MuxOpenedDispatcher",
        "pub(crate) async fn run_mux_session_loop",
        "pub(crate) fn drain_completed_mux_tasks",
        "pub(crate) async fn run_protocol_mux_session",
    ] {
        assert!(
            mux_session.contains(expected),
            "mux_session module tree must still provide `{expected}`"
        );
    }
}

#[test]
fn mux_udp_root_stays_facade_only() {
    let mux_udp_root = read(&proxy_src().join("runtime/mux_udp.rs"));
    let mux_udp = read_module(&proxy_src().join("runtime/mux_udp.rs"));
    for module_name in ["mod handler;", "mod relay;", "mod task;"] {
        assert!(mux_udp_root.contains(module_name));
    }
    for forbidden in [
        "struct MuxPacketSessionUdpHandler",
        "pub(crate) async fn run_protocol_mux_udp_relay",
        "pub(crate) async fn run_protocol_mux_udp_task",
        "pub(crate) async fn run_protocol_mux_udp_task_with_accept_log",
    ] {
        assert!(
            !mux_udp_root.contains(forbidden),
            "mux_udp facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "pub(super) struct MuxPacketSessionUdpHandler",
        "pub(crate) async fn run_protocol_mux_udp_relay",
        "pub(crate) async fn run_protocol_mux_udp_task",
        "pub(crate) async fn run_protocol_mux_udp_task_with_accept_log",
    ] {
        assert!(
            mux_udp.contains(expected),
            "mux_udp module tree must still provide `{expected}`"
        );
    }
}

#[test]
fn no_client_mux_root_stays_facade_only() {
    let no_client_root = read(&proxy_src().join("runtime/inbound_route/mux/no_client.rs"));
    let no_client = read_module(&proxy_src().join("runtime/inbound_route/mux/no_client.rs"));
    for module_name in ["mod request;", "mod route;"] {
        assert!(no_client_root.contains(module_name));
    }
    for forbidden in [
        "pub(crate) async fn dispatch_no_client_mux_route<",
        "pub(crate) async fn dispatch_no_client_mux_route_with_defaults<",
        "pub(crate) async fn dispatch_no_client_mux_route_request_with_defaults<",
    ] {
        assert!(
            !no_client_root.contains(forbidden),
            "no_client mux facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "pub(crate) async fn dispatch_no_client_mux_route<",
        "pub(crate) async fn dispatch_no_client_mux_route_with_defaults<",
        "pub(crate) async fn dispatch_no_client_mux_route_request_with_defaults<",
    ] {
        assert!(
            no_client.contains(expected),
            "no_client mux module tree must still provide `{expected}`"
        );
    }
}

#[test]
fn recorded_dispatch_root_stays_facade_only() {
    let dispatch_root = read(&proxy_src().join("runtime/inbound_route/recorded/dispatch.rs"));
    let dispatch = read_module(&proxy_src().join("runtime/inbound_route/recorded/dispatch.rs"));
    for module_name in ["mod accept;", "mod route;"] {
        assert!(dispatch_root.contains(module_name));
    }
    for forbidden in [
        "pub(crate) async fn dispatch_recorded_protocol_mux_route<",
        "pub(crate) async fn dispatch_recorded_protocol_mux_route_accept_result<",
        "pub(crate) async fn dispatch_optional_recorded_protocol_mux_route_accept_result<",
    ] {
        assert!(
            !dispatch_root.contains(forbidden),
            "recorded dispatch facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "pub(crate) async fn dispatch_recorded_protocol_mux_route<",
        "pub(crate) async fn dispatch_recorded_protocol_mux_route_accept_result<",
        "pub(crate) async fn dispatch_optional_recorded_protocol_mux_route_accept_result<",
    ] {
        assert!(
            dispatch.contains(expected),
            "recorded dispatch module tree must still provide `{expected}`"
        );
    }
}

#[test]
fn recorded_request_root_stays_facade_only() {
    let request_root = read(&proxy_src().join("runtime/inbound_route/recorded/request.rs"));
    let request = read_module(&proxy_src().join("runtime/inbound_route/recorded/request.rs"));
    for module_name in ["mod defaults;", "mod result;"] {
        assert!(request_root.contains(module_name));
    }
    for forbidden in [
        "pub(crate) async fn dispatch_recorded_protocol_mux_tcp_request_result<",
        "pub(crate) async fn dispatch_recorded_protocol_mux_stream_request_result<",
        "pub(crate) async fn dispatch_recorded_protocol_mux_tcp_request_with_defaults<",
        "pub(crate) async fn dispatch_recorded_protocol_mux_stream_request_with_defaults<",
    ] {
        assert!(
            !request_root.contains(forbidden),
            "recorded request facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "pub(crate) async fn dispatch_recorded_protocol_mux_tcp_request_result<",
        "pub(crate) async fn dispatch_recorded_protocol_mux_stream_request_result<",
        "pub(crate) async fn dispatch_recorded_protocol_mux_tcp_request_with_defaults<",
        "pub(crate) async fn dispatch_recorded_protocol_mux_stream_request_with_defaults<",
    ] {
        assert!(
            request.contains(expected),
            "recorded request module tree must still provide `{expected}`"
        );
    }
}

#[test]
fn stream_udp_root_stays_facade_only() {
    let stream_udp_root = read(&proxy_src().join("runtime/stream_udp.rs"));
    let stream_udp = read_module(&proxy_src().join("runtime/stream_udp.rs"));
    for module_name in ["mod handler;", "mod recording;", "mod relay;"] {
        assert!(stream_udp_root.contains(module_name));
    }
    for forbidden in [
        "pub(crate) struct StreamUdpRelayRequest",
        "struct StreamPacketSessionUdpHandler",
        "pub(crate) async fn run_mapped_protocol_stream_udp_relay",
        "async fn run_stream_udp_relay",
        "fn record_stream_udp_client_io",
    ] {
        assert!(
            !stream_udp_root.contains(forbidden),
            "stream_udp facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "pub(crate) struct StreamUdpRelayRequest",
        "pub(super) struct StreamPacketSessionUdpHandler",
        "pub(crate) async fn run_mapped_protocol_stream_udp_relay",
        "async fn run_stream_udp_relay",
        "pub(super) fn record_stream_udp_client_io",
    ] {
        assert!(
            stream_udp.contains(expected),
            "stream_udp module tree must still provide `{expected}`"
        );
    }
}

#[test]
fn udp_flow_sessions_root_stays_facade_only() {
    let sessions_root = read(&proxy_src().join("runtime/udp_flow/sessions.rs"));
    let sessions = read_module(&proxy_src().join("runtime/udp_flow/sessions.rs"));
    for module_name in ["mod index;", "mod lifecycle;", "mod model;"] {
        assert!(sessions_root.contains(module_name));
    }
    for forbidden in [
        "pub(crate) struct UdpSessionFlows",
        "struct UdpFlowKey",
        "fn index_flow(",
        "pub(crate) fn finish_all(&mut self)",
    ] {
        assert!(
            !sessions_root.contains(forbidden),
            "udp_flow sessions facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "pub(crate) struct UdpSessionFlows",
        "pub(crate) struct CompletedUdpFlow",
        "pub(crate) fn finish_all(&mut self)",
        "pub(crate) fn direct_response_session_id(&self, sender: SocketAddr)",
    ] {
        assert!(
            sessions.contains(expected),
            "udp_flow sessions module tree must still provide `{expected}`"
        );
    }
}

#[test]
fn packet_session_udp_lifecycle_root_stays_facade_only() {
    let lifecycle_root = read(&proxy_src().join("runtime/packet_session_udp/lifecycle.rs"));
    let lifecycle = read_module(&proxy_src().join("runtime/packet_session_udp/lifecycle.rs"));
    for module_name in ["mod failure;", "mod read;", "mod relay;", "mod response;"] {
        assert!(lifecycle_root.contains(module_name));
    }
    for forbidden in [
        "pub(crate) async fn run_packet_session_udp_relay",
        "async fn process_packet_session_read",
        "async fn handle_runtime_failure",
        "async fn handle_direct_response",
        "tokio::select!",
    ] {
        assert!(
            !lifecycle_root.contains(forbidden),
            "packet_session_udp lifecycle facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "pub(crate) async fn run_packet_session_udp_relay",
        "async fn process_packet_session_read",
        "async fn handle_runtime_failure",
        "async fn handle_direct_response",
        "select! {",
    ] {
        assert!(
            lifecycle.contains(expected),
            "packet_session_udp lifecycle module tree must still provide `{expected}`"
        );
    }
}

#[test]
fn datagram_udp_lifecycle_root_stays_facade_only() {
    let lifecycle_root = read(&proxy_src().join("runtime/datagram_udp/lifecycle.rs"));
    let lifecycle = read_module(&proxy_src().join("runtime/datagram_udp/lifecycle.rs"));
    for module_name in [
        "mod read;",
        "mod relay;",
        "mod response;",
        "mod without_upstream;",
    ] {
        assert!(lifecycle_root.contains(module_name));
    }
    for forbidden in [
        "pub(crate) async fn run_protocol_datagram_udp_relay",
        "async fn process_datagram_read",
        "type ChainUdpResponseResult",
        "tokio::select!",
    ] {
        assert!(
            !lifecycle_root.contains(forbidden),
            "datagram_udp lifecycle facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "pub(crate) async fn run_protocol_datagram_udp_relay",
        "async fn process_datagram_read",
        "type ChainUdpResponseResult",
        "select! {",
    ] {
        assert!(
            lifecycle.contains(expected),
            "datagram_udp lifecycle module tree must still provide `{expected}`"
        );
    }
}

#[test]
fn udp_association_contract_root_stays_facade_only() {
    let contract_root = read(&proxy_src().join("runtime/udp_association/contract.rs"));
    let contract = read_module(&proxy_src().join("runtime/udp_association/contract.rs"));
    for module_name in [
        "mod dispatch;",
        "mod handler;",
        "mod model;",
        "mod response;",
    ] {
        assert!(contract_root.contains(module_name));
    }
    for forbidden in [
        "enum UdpAssociationDispatchOutcome",
        "struct UdpAssociationDispatchBridge",
        "trait UdpAssociationHandler",
        "async fn write_target_response",
        "async fn send_association_response",
    ] {
        assert!(
            !contract_root.contains(forbidden),
            "udp_association contract facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "enum UdpAssociationDispatchOutcome",
        "struct UdpAssociationDispatchBridge",
        "trait UdpAssociationHandler",
        "async fn write_target_response",
        "async fn send_association_response",
    ] {
        assert!(
            contract.contains(expected),
            "udp_association contract module tree must still provide `{expected}`"
        );
    }
}

#[test]
fn udp_association_lifecycle_root_stays_facade_only() {
    let lifecycle_root = read(&proxy_src().join("runtime/udp_association/lifecycle.rs"));
    let lifecycle = read_module(&proxy_src().join("runtime/udp_association/lifecycle.rs"));
    for module_name in ["mod idle;", "mod relay;", "mod response;"] {
        assert!(lifecycle_root.contains(module_name));
    }
    for forbidden in [
        "pub(crate) async fn run_udp_association_loop",
        "fn handle_idle_timeout(",
        "async fn handle_upstream_response",
        "async fn handle_chain_result",
        "select! {",
    ] {
        assert!(
            !lifecycle_root.contains(forbidden),
            "udp_association lifecycle facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "pub(crate) async fn run_udp_association_loop",
        "pub(super) fn finish_dispatch(",
        "pub(super) async fn handle_upstream_response",
        "pub(super) async fn handle_chain_result",
        "select! {",
    ] {
        assert!(
            lifecycle.contains(expected),
            "udp_association lifecycle module tree must still provide `{expected}`"
        );
    }
}

#[test]
fn udp_flow_state_root_stays_facade_only() {
    let state_root = read(&proxy_src().join("runtime/udp_flow/state.rs"));
    let state = read_module(&proxy_src().join("runtime/udp_flow/state.rs"));
    for module_name in [
        "mod context;",
        "mod lifecycle;",
        "mod managed;",
        "mod model;",
        "mod packet_path;",
    ] {
        assert!(state_root.contains(module_name));
    }
    for forbidden in [
        "pub(crate) struct UdpFlowState",
        "pub(crate) struct UdpFlowStartContext",
        "pub(crate) async fn start_managed_flow(",
        "pub(crate) async fn send_packet_path_chain(",
    ] {
        assert!(
            !state_root.contains(forbidden),
            "udp_flow state facade root must not keep `{forbidden}` inline"
        );
    }
    for expected in [
        "pub(crate) struct UdpFlowState",
        "pub(crate) struct UdpFlowStartContext",
        "pub(crate) async fn start_managed_flow(",
        "pub(crate) async fn send_packet_path_chain(",
    ] {
        assert!(
            state.contains(expected),
            "udp_flow state module tree must still provide `{expected}`"
        );
    }
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
    assert!(outbound.contains("fn outbound_protocol_entry("));
    assert!(!outbound.contains("fn claimed_tcp_outbound_leaf"));
    assert!(!outbound.contains("fn claimed_tcp_entry"));
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
    let build = read(&proxy_src().join("protocol_registry/registry/build.rs"));
    let registry_mod = read(&proxy_src().join("protocol_registry/registry/mod.rs"));
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
    assert!(outbound.contains("leaf.protocol_name()"));
    assert!(outbound.contains("entry.support.name() == protocol"));
    assert!(!outbound.contains("for entry in &self.entries {\n            if let Some(claimed) = entry.tcp.claim_tcp_outbound_leaf(leaf.clone()) {"));
    assert!(!outbound.contains("for entry in &self.entries {\r\n            if let Some(claimed) = entry.tcp.claim_tcp_outbound_leaf(leaf.clone()) {"));
    assert!(capability.contains("struct OutboundLeafClaim<'a>"));
    assert!(!capability.contains("trait OutboundLeafClaimCapability"));
    assert!(registry_mod.contains("trait OutboundLeafClaimer"));
    assert!(build.contains("type OutboundLeafClaimFn"));
    assert!(outbound.contains("entry.outbound.claim_outbound_leaf(leaf.clone())"));
    assert!(outbound.contains("fn claim_outbound_hooks<'a>("));
    assert!(!outbound.contains("claim_tcp_outbound_leaf(leaf.clone())"));
    assert!(!outbound.contains("claim_udp_flow_leaf(leaf.clone())"));
    assert!(!outbound.contains("claim_udp_packet_path_leaf(leaf.clone())"));
    assert!(!outbound.contains("fn claim_tcp_hooks<'a>("));
    assert!(!outbound.contains("fn claim_udp_hooks<'a>("));
    assert!(outbound.contains("struct ClaimedTcpHooks"));
    assert!(outbound.contains("struct ClaimedUdpHooks"));
    assert!(!outbound.contains("HookClaimedTcpLeaf"));
    assert!(!outbound.contains("HookClaimedUdpLeaf"));
    assert!(!outbound.contains("HookClaimedUdpPacketPathLeaf"));
    assert!(!outbound.contains("self.leaf"));
    assert!(!outbound.contains("let runtime = capability.runtime();"));
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
    assert!(!build.contains("adapter.claim_tcp_outbound_leaf("));
    assert!(!build.contains("adapter.claim_udp_flow_leaf("));
    assert!(!build.contains("adapter.claim_udp_packet_path_leaf("));
}

#[test]
fn transport_bridge_adapters_offer_claim_time_tcp_projection() {
    let helper = read(&proxy_src().join("protocol_registry/transport_leaf/tcp.rs"));
    assert!(helper.contains("struct ClaimedTransportTcpLeaf"));
    assert!(helper.contains("claim_transport_tcp_leaf"));

    for relative in [
        "adapters/vless.rs",
        "adapters/vmess.rs",
        "adapters/trojan.rs",
    ] {
        let source = read(&proxy_src().join(relative));
        assert!(
            source.contains("fn claim_outbound_leaf_impl<'a>("),
            "{relative} should expose a unified claim-time outbound projection path"
        );
        assert!(
            source.contains("claim_transport_tcp_leaf("),
            "{relative} should project into the shared claimed transport leaf helper"
        );
    }
}

#[test]
fn transport_bridge_adapters_offer_claim_time_udp_projection() {
    let helper = read(&proxy_src().join("protocol_registry/transport_leaf/udp.rs"));
    assert!(helper.contains("struct ClaimedTransportUdpLeaf"));
    assert!(helper.contains("claim_transport_udp_leaf"));
    assert!(helper.contains("ClaimedRelayTwoStreamTransportUdpLeaf"));
    assert!(helper.contains("claim_relay_two_stream_transport_udp_leaf"));

    for relative in [
        "adapters/vless.rs",
        "adapters/vmess.rs",
        "adapters/trojan.rs",
    ] {
        let source = read(&proxy_src().join(relative));
        assert!(
            source.contains("fn claim_outbound_leaf_impl<'a>("),
            "{relative} should expose a unified claim-time outbound projection path"
        );
    }

    let vless = read(&proxy_src().join("adapters/vless.rs"));
    assert!(vless.contains("claim_relay_two_stream_transport_udp_leaf("));
    assert!(!vless.contains("VlessStreamBridge"));

    for relative in ["adapters/vmess.rs", "adapters/trojan.rs"] {
        let source = read(&proxy_src().join(relative));
        assert!(
            source.contains("claim_transport_udp_leaf("),
            "{relative} should project into the shared claimed UDP transport leaf helper"
        );
    }

    let vmess = read(&proxy_src().join("adapters/vmess.rs"));
    assert!(!vmess.contains("VmessStreamBridge"));

    let trojan = read(&proxy_src().join("adapters/trojan.rs"));
    assert!(!trojan.contains("TrojanTlsBridge"));
}

#[test]
fn managed_stream_udp_handlers_key_off_resume_metadata_not_bridge_types() {
    let transport_managed_udp = read(&workspace_root().join("crates/transport/src/managed_udp.rs"));
    let handler =
        read(&proxy_src().join("runtime/udp_flow/managed/bridge/stream_packet/handler.rs"));
    assert!(!transport_managed_udp.contains("ProtocolManagedStreamUdpBridgeHandlerMetadata"));
    assert!(transport_managed_udp.contains("ProtocolManagedStreamUdpLeafOps"));
    assert!(transport_managed_udp.contains("ProtocolRelayTwoStreamManagedUdpLeafOps"));
    assert!(handler.contains("managed_stream_udp_handler_for_resume"));
    assert!(!handler.contains("managed_stream_udp_handler_for_bridge"));

    for relative in [
        "adapters/vless.rs",
        "adapters/vmess.rs",
        "adapters/trojan.rs",
    ] {
        let source = read(&proxy_src().join(relative));
        assert!(
            source.contains("managed_stream_udp_handler_for_resume::<"),
            "{relative} should register managed stream UDP handlers by resume type"
        );
        assert!(
            !source.contains("managed_stream_udp_handler_for_bridge::<"),
            "{relative} must not route managed stream UDP handler registration through bridge types"
        );
    }
}

#[test]
fn transport_leaf_metadata_owns_live_runtime_stage_constants() {
    let outbound_leaf = read(&workspace_root().join("crates/transport/src/outbound_leaf.rs"));
    assert!(outbound_leaf.contains("pub trait ProtocolTcpTransportLeafMetadata"));
    assert!(outbound_leaf.contains("pub trait ProtocolTcpTransportLeafOps"));
    assert!(outbound_leaf.contains("pub trait ProtocolUdpTransportLeafMetadata"));
    assert!(outbound_leaf.contains("pub trait ProtocolRelayTwoStreamUdpTransportLeafMetadata"));
    assert!(!outbound_leaf.contains("pub trait ProtocolTcpTransportBridgeMetadata"));
    assert!(!outbound_leaf.contains("pub trait ProtocolUdpTransportBridgeMetadata"));
    assert!(!outbound_leaf.contains("pub trait ProtocolRelayTwoStreamUdpTransportBridgeMetadata"));
    assert!(!outbound_leaf.contains("pub trait ProtocolTcpTransportBridgeOps"));
    assert!(!outbound_leaf.contains("TCP_INVALID_CONNECT_LEAF_STAGE"));
    assert!(!outbound_leaf.contains("TCP_INVALID_RELAY_LEAF_STAGE"));
    assert!(!outbound_leaf.contains("EXPECTED_OUTBOUND_LEAF"));

    let vless_leaf = read(&workspace_root().join("protocols/vless/src/transport/leaf.rs"));
    assert!(vless_leaf.contains("impl ProtocolTcpTransportLeafMetadata for VlessOutboundLeaf"));
    assert!(vless_leaf.contains("impl ProtocolUdpTransportLeafMetadata for VlessOutboundLeaf"));
    assert!(vless_leaf
        .contains("impl ProtocolRelayTwoStreamUdpTransportLeafMetadata for VlessOutboundLeaf"));
    assert!(vless_leaf.contains("impl ProtocolTcpTransportLeafOps for VlessOutboundLeaf"));
    assert!(vless_leaf.contains("impl ProtocolManagedStreamUdpLeafOps for VlessOutboundLeaf"));
    assert!(
        vless_leaf.contains("impl ProtocolRelayTwoStreamManagedUdpLeafOps for VlessOutboundLeaf")
    );

    let vmess_leaf = read(&workspace_root().join("protocols/vmess/src/transport/leaf.rs"));
    assert!(vmess_leaf.contains("impl ProtocolTcpTransportLeafMetadata for VmessOutboundLeaf"));
    assert!(vmess_leaf.contains("impl ProtocolUdpTransportLeafMetadata for VmessOutboundLeaf"));
    assert!(vmess_leaf.contains("impl ProtocolTcpTransportLeafOps for VmessOutboundLeaf"));
    assert!(vmess_leaf.contains("impl ProtocolManagedStreamUdpLeafOps for VmessOutboundLeaf"));

    let trojan_leaf = read(&workspace_root().join("protocols/trojan/src/transport/leaf.rs"));
    assert!(trojan_leaf.contains("impl ProtocolTcpTransportLeafMetadata for TrojanOutboundLeaf"));
    assert!(trojan_leaf.contains("impl ProtocolUdpTransportLeafMetadata for TrojanOutboundLeaf"));
    assert!(trojan_leaf.contains("impl ProtocolTcpTransportLeafOps for TrojanOutboundLeaf"));
    assert!(trojan_leaf.contains("impl ProtocolManagedStreamUdpLeafOps for TrojanOutboundLeaf"));

    for relative in [
        "protocols/vless/src/transport.rs",
        "protocols/vmess/src/transport.rs",
        "protocols/trojan/src/transport.rs",
    ] {
        let source = read(&workspace_root().join(relative));
        assert!(
            !source.contains("mod bridge;"),
            "{relative} must not compile a standalone transport bridge module"
        );
        assert!(
            !source.contains("pub use bridge::"),
            "{relative} must not re-export standalone transport bridge types"
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
            source.contains("fn claim_outbound_leaf_impl<'a>("),
            "{relative} should expose a unified claim-time outbound projection path"
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
            source.contains("fn claim_outbound_leaf_impl<'a>("),
            "{relative} should expose a unified claim-time outbound projection path"
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
            source.contains("fn claim_outbound_leaf_impl<'a>("),
            "{relative} should expose a unified claim-time outbound projection path"
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
    let runtime_urltest = read(&proxy_src().join("runtime/listeners/urltest.rs"));
    assert!(urltest.contains("struct UrlTestRuntime"));
    assert!(urltest.contains("dispatch_tcp_outbound("));
    assert!(!urltest.contains("use crate::runtime::Proxy"));
    assert!(!urltest.contains("from_proxy("));
    assert!(!urltest.contains("prepare_tcp_candidate("));
    assert!(!urltest.contains("ResolvedLeafOutbound"));
    assert!(!urltest.contains("impl Proxy"));
    assert!(!runtime_urltest.contains("use super::super::Proxy"));
    assert!(!runtime_urltest.contains("proxy: &Proxy"));
}
