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
    let tcp_operation = read(&proxy_src().join("runtime/tcp_dispatch/operation.rs"));
    let udp_operation = read(&proxy_src().join("runtime/udp_dispatch/operation.rs"));
    assert!(tcp_operation.contains("OutboundAdapterContext"));
    assert!(udp_operation.contains("UdpAdapterContext"));
}

#[test]
fn transport_bridge_operations_are_generic() {
    let tcp = read(&proxy_src().join("runtime/tcp_dispatch/operation.rs"));
    let udp = read(&proxy_src().join("runtime/udp_dispatch/operation.rs"));
    assert!(tcp.contains("TransportBridgeTcpConnectOperation"));
    assert!(tcp.contains("TransportBridgeTcpRelayOperation"));
    assert!(tcp.contains("PreparedTransportBridgeLeaf"));
    assert!(udp.contains("TransportBridgeUdpOperation"));
    assert!(udp.contains("RelayTwoStreamUdpOperation"));
}

#[test]
fn protocol_crates_do_not_depend_on_proxy_or_runtime_crates() {
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
        for forbidden in [
            "zero-proxy",
            "zero-engine",
            "zero-transport",
            "zero-platform-tokio",
        ] {
            assert!(
                !source.contains(forbidden),
                "{} must remain runtime-neutral and not depend on `{forbidden}`",
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
}

#[test]
fn adapter_runtime_service_access_does_not_expose_proxy() {
    let context = read(&proxy_src().join("protocol_registry/context.rs"));
    assert!(context.contains("struct UdpRuntimeServices"));
    assert!(context.contains("fn runtime_services"));
    let adapters = rust_sources(&proxy_src().join("adapters"))
        .into_iter()
        .map(|path| read(&path))
        .collect::<String>();
    assert!(!adapters.contains("ctx.proxy()"));
    assert!(!adapters.contains("Proxy {"));
}
