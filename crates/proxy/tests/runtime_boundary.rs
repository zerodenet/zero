use std::fs;
use std::path::{Path, PathBuf};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repo_root() -> PathBuf {
    manifest_dir()
        .parent()
        .expect("proxy crate parent")
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn read(relative: &str) -> String {
    let path = manifest_dir().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn rust_sources_under(relative: &str) -> Vec<PathBuf> {
    let root = manifest_dir().join(relative);
    let mut pending = vec![root];
    let mut files = Vec::new();

    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(&path).unwrap_or_else(|error| {
            panic!("read dir {}: {error}", path.display());
        }) {
            let entry = entry.expect("read dir entry");
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                files.push(path);
            }
        }
    }

    files
}

fn relative(path: &Path) -> String {
    path.strip_prefix(manifest_dir())
        .expect("path under manifest dir")
        .to_string_lossy()
        .replace('\\', "/")
}

fn assert_src_pattern_confined(
    pattern: &str,
    allowed_exact: &[&str],
    allowed_prefixes: &[&str],
    reason: &str,
) {
    for path in rust_sources_under("src") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        if !content.contains(pattern) {
            continue;
        }
        let allowed = allowed_exact.iter().any(|item| *item == source)
            || allowed_prefixes
                .iter()
                .any(|prefix| source.starts_with(prefix));
        assert!(allowed, "{source} should not contain `{pattern}`; {reason}");
    }
}

#[test]
fn proxy_production_sources_do_not_keep_todo_markers() {
    for path in rust_sources_under("src") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        for marker in ["TODO", "FIXME"] {
            assert!(
                !content.contains(marker),
                "{source} should not keep unresolved `{marker}` markers in production code"
            );
        }
    }
}

#[test]
fn protocol_identity_parsing_is_confined_to_adapters() {
    for path in rust_sources_under("src") {
        let source = relative(&path);
        if source.starts_with("src/adapters/") {
            continue;
        }
        let content = fs::read_to_string(&path).expect("read rust source");
        for forbidden in [
            "parse_uuid",
            "VmessCipher::from_name",
            "CipherKind::from_str",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should not parse protocol identity/cipher config outside adapters; found `{forbidden}`"
            );
        }
    }
}

#[test]
fn runtime_protocol_runtime_references_are_confined_to_facades() {
    let allowed_exact = [
        "src/runtime/udp_dispatch/mod.rs",
        "src/runtime/udp_dispatch/hysteria2_flow.rs",
        "src/runtime/udp_dispatch/lifecycle.rs",
        "src/runtime/udp_dispatch/mieru_flow.rs",
        "src/runtime/udp_dispatch/shadowsocks_flow.rs",
        "src/runtime/udp_dispatch/socks5_flow.rs",
        "src/runtime/udp_dispatch/trojan_flow.rs",
        "src/runtime/udp_dispatch/vless_flow.rs",
        "src/runtime/udp_dispatch/vmess_flow.rs",
        "src/runtime/udp_dispatch/start/relay.rs",
        "src/runtime/udp_flow/outbound.rs",
    ];

    for path in rust_sources_under("src/runtime") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read runtime source");
        if !content.contains("crate::protocol_runtime::") {
            continue;
        }
        assert!(
            allowed_exact.iter().any(|allowed| *allowed == source),
            "{source} should not reference protocol_runtime directly; add a narrow facade or extend this allow-list with a boundary test"
        );
    }
}

#[test]
fn ordinary_udp_inbounds_submit_packets_through_udp_pipe() {
    for source in [
        "src/protocol_runtime/socks5_udp_associate/dispatch.rs",
        "src/inbound/vless/udp_session.rs",
        "src/inbound/trojan.rs",
        "src/inbound/shadowsocks/udp.rs",
        "src/inbound/hysteria2.rs",
    ] {
        let content = read(source);
        assert!(
            content.contains("UdpPipe::new") && content.contains("UdpPipeInput"),
            "{source} should submit inbound UDP packets through UdpPipe"
        );
        assert!(
            !content.contains("UdpDispatch::dispatch"),
            "{source} should not call the UDP dispatch state machine directly"
        );
    }
}

#[test]
fn udp_dispatch_entry_is_only_called_by_udp_pipe() {
    for path in rust_sources_under("src") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        if source == "src/runtime/pipe.rs" {
            continue;
        }
        assert!(
            !content.contains("UdpDispatch::dispatch"),
            "{source} should not call UdpDispatch::dispatch directly"
        );
    }
}

#[test]
fn ordinary_tcp_inbounds_use_tcp_pipe_for_route_execution() {
    let lifecycle = read("src/runtime/inbound_protocol.rs");
    assert!(
        lifecycle.contains("TcpPipe::new") && lifecycle.contains("TcpPipeInput"),
        "serve_inbound should route ordinary TCP sessions through TcpPipe"
    );

    let vless = read("src/inbound/vless/mux.rs");
    assert!(
        vless.contains("TcpPipe::new") && vless.contains("TcpPipeInput"),
        "VLESS MUX sub-streams should route through TcpPipe"
    );
    assert!(
        !vless.contains("dispatch_tcp_outbound"),
        "VLESS inbound should not bypass TcpPipe through TCP outbound helpers"
    );
}

#[test]
fn tcp_outbound_resolution_helper_stays_inside_tcp_dispatch() {
    for path in rust_sources_under("src") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        if source == "src/runtime/tcp_dispatch.rs" {
            continue;
        }
        assert!(
            !content.contains("dispatch_tcp_outbound"),
            "{source} should not call the TCP outbound helper directly"
        );
    }
}

#[test]
fn protocol_config_variant_matching_is_confined_to_adapters_and_protocol_entrypoints() {
    assert_src_pattern_confined(
        "InboundProtocolConfig::",
        &[
            "src/protocol_adapter.rs",
            "src/protocol_adapter/registry.rs",
            "src/protocol_adapter/registry/metadata.rs",
            "src/protocol_adapter/registry/tests.rs",
            "src/protocol_adapter/registry/tests/fixtures.rs",
        ],
        &["src/adapters/"],
        "protocol config variant matching should stay inside adapters or protocol-owned inbound entrypoints",
    );
}

#[test]
fn outbound_config_variant_matching_is_confined_to_adapters_and_registry() {
    assert_src_pattern_confined(
        "OutboundProtocolConfig::",
        &[
            "src/protocol_adapter/registry.rs",
            "src/protocol_adapter/registry/support.rs",
        ],
        &["src/adapters/"],
        "outbound config variant matching should stay inside adapters or protocol registry feature helpers",
    );
}

#[test]
fn direct_udp_helpers_do_not_live_in_outbound_facade() {
    let outbound_root = read("src/outbound/mod.rs");
    assert!(
        !outbound_root.contains("mod direct") && !outbound_root.contains("pub mod direct"),
        "direct UDP helpers should live in runtime::udp_helpers and direct adapter modules, not src/outbound/direct.rs"
    );
    assert!(
        !manifest_dir().join("src/outbound/direct.rs").exists(),
        "src/outbound/direct.rs should not be kept as an empty compatibility facade"
    );

    let helpers = read("src/runtime/udp_helpers.rs");
    let adapter = read("src/adapters/direct/udp.rs");
    assert!(
        helpers.contains("resolve_udp_target") && helpers.contains("send_direct_udp_packet"),
        "direct UDP target resolution and sending should live in runtime::udp_helpers"
    );
    assert!(
        !helpers.contains("outbound/direct.rs"),
        "runtime::udp_helpers should not keep historical references to removed outbound direct facades"
    );
    assert!(
        adapter.contains("resolve_udp_target") && adapter.contains("send_direct_packet"),
        "direct adapter UDP module should call runtime helpers through UdpDispatch"
    );
}

#[test]
fn outbound_protocol_helpers_are_crate_private() {
    let outbound_root = read("src/outbound/mod.rs");

    for protocol in [
        "hysteria2",
        "mieru",
        "shadowsocks",
        "socks5",
        "trojan",
        "vless",
        "vmess",
    ] {
        assert!(
            !outbound_root.contains(&format!("pub mod {protocol};")),
            "src/outbound/mod.rs should not expose `{protocol}` helpers as public modules"
        );
        assert!(
            outbound_root.contains(&format!("pub(crate) mod {protocol};")),
            "src/outbound/mod.rs should keep `{protocol}` helpers crate-private"
        );
    }
}

#[test]
fn outbound_root_is_facade_only() {
    let outbound_root = read("src/outbound/mod.rs");

    for expected in [
        "pub(crate) mod hysteria2;",
        "pub(crate) mod mieru;",
        "pub(crate) mod shadowsocks;",
        "pub(crate) mod socks5;",
        "pub(crate) mod trojan;",
        "pub(crate) mod vless;",
        "pub(crate) mod vmess;",
    ] {
        assert!(
            outbound_root.contains(expected),
            "src/outbound/mod.rs should expose outbound facade item `{expected}`"
        );
    }

    for line in outbound_root.lines().map(str::trim) {
        let allowed =
            line.is_empty() || line.starts_with("#[cfg(") || line.starts_with("pub(crate) mod ");
        assert!(
            allowed,
            "src/outbound/mod.rs should only declare crate-private outbound helper modules; found `{line}`"
        );
    }

    for forbidden in [
        "pub mod ",
        "pub(crate) use ",
        "async fn",
        "fn ",
        "impl ",
        "match ",
        "InboundProtocolConfig::",
        "OutboundProtocolConfig::",
        "ResolvedLeafOutbound::",
    ] {
        assert!(
            !outbound_root.contains(forbidden),
            "src/outbound/mod.rs should remain a facade over outbound helper modules; found `{forbidden}`"
        );
    }
}

#[test]
fn runtime_does_not_match_protocol_config_variants() {
    for path in rust_sources_under("src/runtime") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        assert!(
            !content.contains("InboundProtocolConfig::"),
            "{source} should not match inbound protocol config variants"
        );
    }

    let runtime = read("src/runtime.rs");
    assert!(
        !runtime.contains("InboundProtocolConfig::"),
        "src/runtime.rs should dispatch inbound lifecycle through ProtocolAdapter"
    );
}

#[test]
fn runtime_does_not_resolve_inbound_adapter_objects() {
    let listeners = read("src/runtime/listeners.rs");
    let inventory_inbound = read("src/inventory/inbound.rs");

    for forbidden in ["find_inbound", "adapter.spawn_inbound"] {
        assert!(
            !listeners.contains(forbidden),
            "src/runtime/listeners.rs should ask ProtocolInventory to spawn inbounds without resolving adapter objects; found `{forbidden}`"
        );
    }
    assert!(
        inventory_inbound.contains("pub(crate) fn spawn_inbound(")
            && inventory_inbound.contains("self.registry.find_inbound(&inbound.protocol)?")
            && inventory_inbound.contains("InboundListenerCapability::spawn_inbound("),
        "src/inventory/inbound.rs should own inbound adapter resolution and spawn dispatch"
    );
}

#[test]
fn tcp_runtime_does_not_resolve_outbound_adapter_objects() {
    let tcp_dispatch = read("src/runtime/tcp_dispatch.rs");
    let inventory_tcp = read("src/inventory/tcp.rs");

    for forbidden in ["find_outbound_leaf", ".connect_tcp(", ".apply_relay_hop("] {
        assert!(
            !tcp_dispatch.contains(forbidden),
            "src/runtime/tcp_dispatch.rs should ask ProtocolInventory to drive TCP adapters without resolving adapter objects; found `{forbidden}`"
        );
    }
    assert!(
        inventory_tcp.contains("pub(crate) async fn connect_tcp_leaf(")
            && inventory_tcp.contains("TcpOutboundCapability::connect_tcp(")
            && inventory_tcp.contains("pub(crate) async fn apply_tcp_relay_hop(")
            && inventory_tcp.contains("TcpOutboundCapability::apply_relay_hop("),
        "src/inventory/tcp.rs should own TCP outbound adapter resolution and dispatch"
    );
}

#[test]
fn tcp_runtime_does_not_match_protocol_outbound_results() {
    let tcp_dispatch = read("src/runtime/tcp_dispatch.rs");
    let tcp_outbound = read("src/transport/tcp_outbound.rs");

    for forbidden in [
        "EstablishedTcpOutbound::Socks5",
        "EstablishedTcpOutbound::Vless",
        "EstablishedTcpOutbound::Hysteria2",
        "EstablishedTcpOutbound::Shadowsocks",
        "EstablishedTcpOutbound::Trojan",
        "EstablishedTcpOutbound::Vmess",
        "EstablishedTcpOutbound::Mieru",
    ] {
        assert!(
            !tcp_dispatch.contains(forbidden),
            "src/runtime/tcp_dispatch.rs should not unpack protocol TCP outbound variants; found `{forbidden}`"
        );
        assert!(
            tcp_outbound.contains(forbidden),
            "src/transport/tcp_outbound.rs should own TCP outbound result normalization for `{forbidden}`"
        );
    }

    assert!(
        tcp_dispatch.contains(".into_relay_stream()"),
        "src/runtime/tcp_dispatch.rs should ask EstablishedTcpOutbound for a neutral relay stream"
    );
}

#[test]
fn udp_single_hop_runtime_does_not_resolve_outbound_adapter_objects() {
    let udp_start = read("src/runtime/udp_dispatch/start/mod.rs");
    let inventory_udp_leaf = read("src/inventory/udp/leaf.rs");

    for forbidden in ["find_outbound_leaf", ".start_udp_flow("] {
        assert!(
            !udp_start.contains(forbidden),
            "src/runtime/udp_dispatch/start/mod.rs should ask ProtocolInventory to drive single-hop UDP adapters without resolving adapter objects; found `{forbidden}`"
        );
    }
    assert!(
        inventory_udp_leaf.contains("pub(crate) async fn start_udp_leaf_flow(")
            && inventory_udp_leaf.contains("UdpFlowCapability::start_udp_flow("),
        "src/inventory/udp/leaf.rs should own single-hop UDP adapter resolution and dispatch"
    );
}

#[test]
fn udp_relay_runtime_does_not_resolve_final_hop_adapter_objects() {
    let relay = read("src/runtime/udp_dispatch/start/relay.rs");
    let inventory_udp_relay = read("src/inventory/udp/relay.rs");

    for forbidden in [
        "adapter.udp_relay_needs_two_streams(",
        "adapter.start_udp_relay_two_stream(",
        "adapter.start_udp_relay_final_hop(",
        "find_outbound_leaf(chain.last()",
    ] {
        assert!(
            !relay.contains(forbidden),
            "src/runtime/udp_dispatch/start/relay.rs should ask ProtocolInventory to drive UDP relay final-hop adapters; found `{forbidden}`"
        );
    }
    assert!(
        inventory_udp_relay.contains("pub(crate) fn udp_relay_needs_two_streams(")
            && inventory_udp_relay.contains("pub(crate) async fn start_udp_relay_two_stream(")
            && inventory_udp_relay.contains("pub(crate) async fn start_udp_relay_final_hop("),
        "src/inventory/udp/relay.rs should own UDP relay final-hop adapter resolution and dispatch"
    );
}

#[test]
fn udp_relay_runtime_does_not_resolve_packet_path_pair_adapters() {
    let relay = read("src/runtime/udp_dispatch/start/relay.rs");
    let inventory_udp_packet_path = read("src/inventory/udp/packet_path.rs");

    for forbidden in [
        "carrier_adapter",
        "datagram_adapter",
        "udp_packet_path_carrier_descriptor(",
        "udp_datagram_source(",
    ] {
        assert!(
            !relay.contains(forbidden),
            "src/runtime/udp_dispatch/start/relay.rs should ask ProtocolInventory to classify packet-path pairs; found `{forbidden}`"
        );
    }
    assert!(
        inventory_udp_packet_path.contains("pub(crate) fn udp_packet_path_pair")
            && inventory_udp_packet_path
                .contains("UdpPacketPathCapability::udp_packet_path_carrier_descriptor")
            && inventory_udp_packet_path.contains("UdpPacketPathCapability::udp_datagram_source")
            && inventory_udp_packet_path.contains("PacketPathFlowBinding::new"),
        "src/inventory/udp/packet_path.rs should own packet-path pair adapter probing"
    );
    assert!(
        relay.contains("flow_binding")
            && !relay.contains("flow_snapshot")
            && !relay.contains("packet_path_carrier"),
        "UDP relay start should treat packet-path pair output as one neutral binding"
    );
}

#[test]
fn packet_path_dispatch_is_not_feature_gated_by_datagram_protocol() {
    for source in [
        "src/protocol_adapter/capability.rs",
        "src/inventory/udp/packet_path.rs",
        "src/runtime/udp_dispatch/start/relay.rs",
    ] {
        let content = read(source);
        assert!(
            !content.contains(r#"#[cfg(feature = "shadowsocks")]"#),
            "{source} should expose generic packet-path dispatch independently of the current datagram protocol feature"
        );
    }
}

#[test]
fn packet_path_entry_does_not_resolve_adapter_objects() {
    let entry = read("src/protocol_runtime/udp/packet_path_chain/entry.rs");
    let inventory_udp_packet_path = read("src/inventory/udp/packet_path.rs");

    for forbidden in [
        "find_outbound_leaf",
        "carrier_adapter",
        "datagram_adapter",
        "udp_packet_path_carrier_descriptor(",
        "udp_datagram_source(",
        ".build_udp_packet_path(",
    ] {
        assert!(
            !entry.contains(forbidden),
            "packet_path_chain/entry.rs should ask ProtocolInventory to resolve packet-path adapters; found `{forbidden}`"
        );
    }
    assert!(
        inventory_udp_packet_path.contains("pub(crate) fn resolve_udp_packet_path_candidate")
            && inventory_udp_packet_path
                .contains("pub(crate) async fn build_udp_packet_path_carrier")
            && inventory_udp_packet_path
                .contains("UdpPacketPathCapability::build_udp_packet_path("),
        "src/inventory/udp/packet_path.rs should own packet-path carrier adapter resolution"
    );
}

#[test]
fn mixed_inbound_is_adapter_owned_not_runtime_special_case() {
    let adapter = read("src/adapters/mixed/inbound.rs");
    assert!(
        adapter.contains("run_mixed_listener_with_bound") && adapter.contains("bound.into_tcp()"),
        "MixedAdapter should own mixed listener spawn through the adapter inbound module"
    );

    for path in rust_sources_under("src/runtime") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        for forbidden in [
            "InboundProtocolConfig::Mixed",
            "run_mixed_listener_with_bound",
            "MixedAdapter",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should not special-case mixed inbound; found `{forbidden}`"
            );
        }
    }
}

#[test]
fn runtime_control_handle_lives_outside_runtime_root() {
    let runtime = read("src/runtime.rs");
    let handle = read("src/runtime/handle.rs");

    for forbidden in [
        "struct ProxyHandle",
        "impl zero_api::QueryService for ProxyHandle",
        "impl zero_api::CommandService for ProxyHandle",
        "impl zero_api::EventSource for ProxyHandle",
        "fn parse_ip_address",
    ] {
        assert!(
            !runtime.contains(forbidden),
            "src/runtime.rs should keep control-plane handle details in src/runtime/handle.rs; found `{forbidden}`"
        );
    }

    for required in [
        "struct ProxyHandle",
        "impl zero_api::QueryService for ProxyHandle",
        "impl zero_api::CommandService for ProxyHandle",
        "impl zero_api::EventSource for ProxyHandle",
        "fn parse_ip_address",
    ] {
        assert!(
            handle.contains(required),
            "runtime control-plane handle details should live in src/runtime/handle.rs; missing `{required}`"
        );
    }
}

#[test]
fn running_proxy_handle_lives_outside_runtime_root() {
    let runtime = read("src/runtime.rs");
    let running = read("src/runtime/running.rs");

    for forbidden in [
        "pub struct RunningProxy",
        "impl RunningProxy",
        "impl Deref for RunningProxy",
    ] {
        assert!(
            !runtime.contains(forbidden),
            "src/runtime.rs should keep RunningProxy handle details in src/runtime/running.rs; found `{forbidden}`"
        );
        assert!(
            running.contains(forbidden),
            "src/runtime/running.rs should own RunningProxy handle detail `{forbidden}`"
        );
    }

    assert!(
        runtime.contains("mod running;") && runtime.contains("pub use running::RunningProxy;"),
        "src/runtime.rs should expose RunningProxy through the runtime/running.rs module"
    );
}

#[test]
fn runtime_reload_bridge_lives_outside_runtime_root() {
    let runtime = read("src/runtime.rs");
    let reload = read("src/runtime/reload.rs");

    for forbidden in [
        "spawn_blocking",
        "recv_timeout",
        "unbounded_channel",
        "RecvTimeoutError",
    ] {
        assert!(
            !runtime.contains(forbidden),
            "src/runtime.rs should keep reload bridge details in src/runtime/reload.rs; found `{forbidden}`"
        );
        assert!(
            reload.contains(forbidden),
            "src/runtime/reload.rs should own reload bridge detail `{forbidden}`"
        );
    }

    assert!(
        runtime.contains("mod reload;")
            && runtime.contains("reload::subscribe_reload_bridge(self.engine.subscribe_reload())"),
        "src/runtime.rs should subscribe to reloads through runtime/reload.rs"
    );
}

#[test]
fn proxy_does_not_own_protocol_listener_entrypoints() {
    for path in rust_sources_under("src/inbound") {
        let source = relative(&path);
        let content = fs::read_to_string(&path)
            .expect("read rust source")
            .replace("\r\n", "\n");

        assert!(
            !content.contains("_listener_with_bound(\n")
                || !content.contains("impl Proxy {\n    pub(crate) async fn run_"),
            "{source} should expose run_*_listener_with_bound as module functions, not Proxy methods"
        );
    }
}

#[test]
fn inbound_root_is_facade_only() {
    let root = read("src/inbound/mod.rs");

    for expected in [
        "pub(crate) mod direct;",
        "mod http_connect;",
        "pub(crate) mod hysteria2;",
        "pub(crate) mod mieru;",
        "mod mixed;",
        "pub(crate) mod shadowsocks;",
        "mod socks5;",
        "mod system;",
        "pub(crate) mod trojan;",
        "mod tun;",
        "pub(crate) mod vless;",
        "pub(crate) mod vmess;",
        "pub(crate) use direct::run_direct_listener_with_bound;",
        "pub(crate) use http_connect::run_http_connect_listener_with_bound;",
        "pub(crate) use hysteria2::run_hysteria2_listener_with_bound;",
        "pub(crate) use mieru::run_mieru_listener_with_bound;",
        "pub(crate) use mixed::run_mixed_listener_with_bound;",
        "pub(crate) use shadowsocks::run_shadowsocks_listener_with_bound;",
        "pub(crate) use socks5::run_socks5_listener_with_bound;",
        "pub(crate) use trojan::run_trojan_listener_with_bound;",
        "pub(crate) use vless::run_vless_listener_with_bound;",
        "pub(crate) use vmess::run_vmess_listener_with_bound;",
    ] {
        assert!(
            root.contains(expected),
            "src/inbound/mod.rs should expose inbound facade item `{expected}`"
        );
    }

    for line in root.lines().map(str::trim) {
        let allowed = line.is_empty()
            || line.starts_with("#[cfg(")
            || line.starts_with("mod ")
            || line.starts_with("pub(crate) mod ")
            || line.starts_with("pub(crate) use ");
        assert!(
            allowed,
            "src/inbound/mod.rs should only declare inbound modules and re-export listener entrypoints; found `{line}`"
        );
    }

    for forbidden in [
        "async fn",
        "fn ",
        "impl ",
        "match ",
        "InboundProtocolConfig::",
        "OutboundProtocolConfig::",
        "ResolvedLeafOutbound::",
        "ProtocolAdapter",
    ] {
        assert!(
            !root.contains(forbidden),
            "src/inbound/mod.rs should remain a facade over inbound listener modules; found `{forbidden}`"
        );
    }
}

#[test]
fn resolved_outbound_variant_matching_is_confined_to_adapters_and_registry() {
    assert_src_pattern_confined(
        "ResolvedLeafOutbound::",
        &[
            "src/protocol_adapter.rs",
            "src/protocol_adapter/registry.rs",
            "src/protocol_adapter/registry/outbound.rs",
            "src/protocol_adapter/registry/tests.rs",
            "src/protocol_adapter/registry/tests/fixtures.rs",
            "src/protocol_adapter/registry/tests/outbound.rs",
        ],
        &["src/adapters/"],
        "resolved outbound variant matching should stay inside adapters or protocol registry dispatch helpers",
    );
}

#[test]
fn block_outbound_leaf_is_registry_kernel_exception_not_adapter_protocol() {
    let outbound = read("src/protocol_adapter/registry/outbound.rs");
    assert!(
        outbound.contains("ResolvedLeafOutbound::Block")
            && outbound.contains("TcpPathCategory::Block"),
        "ProtocolRegistry outbound dispatch should own the kernel-level Block leaf classification"
    );

    for path in rust_sources_under("src/adapters") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        assert!(
            !content.contains("ResolvedLeafOutbound::Block")
                && !content.contains("TcpPathCategory::Block"),
            "{source} should not model block as an adapter-owned protocol"
        );
    }

    for path in rust_sources_under("src/runtime") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        assert!(
            !content.contains("ResolvedLeafOutbound::Block"),
            "{source} should get block facts from ProtocolRegistry::outbound_leaf_runtime"
        );
    }
}

#[test]
fn runtime_uses_registry_for_outbound_leaf_runtime_facts() {
    let orchestration = read("src/runtime/orchestration.rs");
    for forbidden in [
        "ResolvedLeafOutbound::",
        "fn health_tag",
        "fn endpoint",
        "fn kernel_leaf_tag",
        "fn tcp_path_category",
    ] {
        assert!(
            !orchestration.contains(forbidden),
            "runtime/orchestration.rs should only define neutral fact types, not classify outbound leaf variants; found `{forbidden}`"
        );
    }

    for path in rust_sources_under("src/runtime") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        assert!(
            !content.contains("ResolvedLeafOutbound::"),
            "{source} should use ProtocolRegistry::outbound_leaf_runtime instead of matching outbound leaf variants"
        );
    }
}

#[test]
fn direct_inbound_uses_adapter_request_model() {
    let inbound = read("src/inbound/direct.rs");
    let adapter = read("src/adapters/direct/inbound.rs");

    assert!(
        inbound.contains("struct DirectInboundRequest")
            && inbound.contains("request: DirectInboundRequest"),
        "direct inbound listener should receive an adapter-built request model"
    );
    assert!(
        !inbound.contains("InboundProtocolConfig::Direct"),
        "direct inbound entrypoint should not parse direct config variants"
    );
    assert!(
        adapter.contains("InboundProtocolConfig::Direct")
            && adapter.contains("DirectInboundRequest"),
        "direct adapter should extract direct config and pass DirectInboundRequest"
    );
}

#[test]
fn mieru_inbound_uses_adapter_request_model() {
    let inbound = read("src/inbound/mieru.rs");
    let adapter = read("src/adapters/mieru/inbound.rs");

    assert!(
        inbound.contains("struct MieruInboundRequest")
            && inbound.contains("request: MieruInboundRequest"),
        "Mieru inbound listener should receive an adapter-built request model"
    );
    assert!(
        !inbound.contains("InboundProtocolConfig::Mieru"),
        "Mieru inbound entrypoint should not parse Mieru config variants"
    );
    assert!(
        adapter.contains("InboundProtocolConfig::Mieru") && adapter.contains("MieruInboundRequest"),
        "Mieru adapter should extract Mieru config and pass MieruInboundRequest"
    );
}

#[test]
fn shadowsocks_inbound_uses_adapter_request_model() {
    let inbound = read("src/inbound/shadowsocks.rs");
    let udp = read("src/inbound/shadowsocks/udp.rs");
    let adapter = read("src/adapters/shadowsocks/inbound.rs");

    assert!(
        inbound.contains("struct ShadowsocksInboundRequest")
            && inbound.contains("request: ShadowsocksInboundRequest"),
        "Shadowsocks inbound listener should receive an adapter-built request model"
    );
    assert!(
        !inbound.contains("InboundProtocolConfig::Shadowsocks"),
        "Shadowsocks inbound entrypoint should not parse Shadowsocks config variants"
    );
    assert!(
        adapter.contains("InboundProtocolConfig::Shadowsocks")
            && adapter.contains("ShadowsocksInboundRequest"),
        "Shadowsocks adapter should extract Shadowsocks config and pass ShadowsocksInboundRequest"
    );
    assert!(
        inbound.contains("pub(crate) cipher: CipherKind")
            && inbound.contains("pub(crate) cipher_name: String")
            && !inbound.contains("CipherKind::from_str"),
        "Shadowsocks inbound listener should receive an adapter-parsed CipherKind plus display name"
    );
    assert!(
        adapter.contains("CipherKind::from_str"),
        "Shadowsocks adapter should parse Shadowsocks cipher config before calling the listener"
    );
    assert!(
        !inbound.contains("#[allow(clippy::too_many_lines)]"),
        "Shadowsocks inbound listener should stay small enough without a too_many_lines allowance"
    );
    assert!(
        !inbound.contains("async fn ss_udp_relay_loop")
            && !inbound.contains("struct SsEncryptedResponse"),
        "Shadowsocks UDP relay details should live outside the listener entrypoint"
    );
    assert!(
        udp.contains("async fn ss_udp_relay_loop")
            && udp.contains("struct SsEncryptedResponse")
            && udp.contains("UdpPipe::new"),
        "Shadowsocks UDP relay should live in src/inbound/shadowsocks/udp.rs and route through UdpPipe"
    );
}

#[test]
fn trojan_inbound_uses_adapter_request_model() {
    let inbound = read("src/inbound/trojan.rs");
    let adapter = read("src/adapters/trojan/inbound.rs");

    assert!(
        inbound.contains("struct TrojanInboundRequest")
            && inbound.contains("request: TrojanInboundRequest"),
        "Trojan inbound listener should receive an adapter-built request model"
    );
    assert!(
        !inbound.contains("InboundProtocolConfig::Trojan"),
        "Trojan inbound entrypoint should not parse Trojan config variants"
    );
    assert!(
        adapter.contains("InboundProtocolConfig::Trojan")
            && adapter.contains("TrojanInboundRequest"),
        "Trojan adapter should extract Trojan config and pass TrojanInboundRequest"
    );
}

#[test]
fn vmess_inbound_uses_adapter_request_model() {
    let inbound = read("src/inbound/vmess/listener.rs");
    let model = read("src/inbound/vmess/model.rs");
    let root = read("src/inbound/vmess/mod.rs");
    let adapter = read("src/adapters/vmess/inbound.rs");

    assert!(
        model.contains("struct VmessInboundRequest")
            && inbound.contains("request: VmessInboundRequest"),
        "VMess inbound listener should receive an adapter-built request model"
    );
    assert!(
        !inbound.contains("InboundProtocolConfig::Vmess")
            && !root.contains("InboundProtocolConfig::Vmess"),
        "VMess inbound entrypoint should not parse VMess config variants"
    );
    assert!(
        adapter.contains("InboundProtocolConfig::Vmess") && adapter.contains("VmessInboundRequest"),
        "VMess adapter should extract VMess config and pass VmessInboundRequest"
    );
    for forbidden in [
        "parse_uuid",
        "VmessCipher::from_name",
        "vmess unknown cipher",
    ] {
        assert!(
            !inbound.contains(forbidden) && !model.contains(forbidden),
            "VMess inbound listener/model should receive adapter-parsed users; found `{forbidden}`"
        );
        assert!(
            adapter.contains(forbidden),
            "VMess adapter should own user parsing detail `{forbidden}`"
        );
    }
    assert!(
        !inbound.contains("VmessUserConfig") && !model.contains("VmessUserConfig"),
        "VMess inbound listener/model should not carry raw config user records"
    );
}

#[test]
fn vless_inbound_users_are_adapter_parsed() {
    let listener = read("src/inbound/vless/listener.rs");
    let model = read("src/inbound/vless/model.rs");
    let session = read("src/inbound/vless/session.rs");
    let helpers = read("src/inbound/vless/helpers.rs");
    let adapter = read("src/adapters/vless/inbound.rs");

    for forbidden in [
        "VlessUserConfig",
        "parse_uuid",
        "parse_flow",
        "vless_users()",
    ] {
        assert!(
            !listener.contains(forbidden)
                && !session.contains(forbidden)
                && !helpers.contains(forbidden),
            "VLESS inbound listener/session/user store should receive adapter-parsed users; found `{forbidden}`"
        );
    }
    for required in [
        "parse_inbound_users",
        "parse_uuid",
        "parse_flow",
        "VlessUser {",
    ] {
        assert!(
            adapter.contains(required),
            "VLESS adapter inbound module should own parsed user construction detail `{required}`"
        );
    }
    assert!(
        helpers.contains("struct ConfiguredVlessUser")
            && helpers.contains("user: VlessUser")
            && helpers.contains("user.user.clone()"),
        "VLESS user store should look up pre-parsed protocol users"
    );
    assert!(
        model.contains("struct VlessInboundRequest")
            && listener.contains("request: VlessInboundRequest"),
        "VLESS inbound request model should live in inbound/vless/model.rs"
    );
}

#[test]
fn hysteria2_inbound_uses_adapter_request_model() {
    let inbound = read("src/inbound/hysteria2.rs");
    let adapter = read("src/adapters/hysteria2/inbound.rs");
    let protocol_udp = fs::read_to_string(repo_root().join("protocols/hysteria2/src/udp.rs"))
        .expect("read hysteria2 protocol udp source");

    assert!(
        inbound.contains("struct Hysteria2InboundRequest")
            && inbound.contains("request: Hysteria2InboundRequest"),
        "Hysteria2 inbound listener should receive an adapter-built request model"
    );
    assert!(
        !inbound.contains("InboundProtocolConfig::Hysteria2"),
        "Hysteria2 inbound entrypoint should not parse Hysteria2 config variants"
    );
    assert!(
        adapter.contains("InboundProtocolConfig::Hysteria2")
            && adapter.contains("Hysteria2InboundRequest"),
        "Hysteria2 adapter should extract Hysteria2 config and pass Hysteria2InboundRequest"
    );
    for forbidden in [
        "build_udp_datagram",
        "parse_udp_datagram",
        "hysteria2::build_udp_datagram",
        "hysteria2::parse_udp_datagram",
        "hysteria2::decode_inbound_udp_datagram",
        "hysteria2::encode_inbound_udp_datagram",
    ] {
        assert!(
            !inbound.contains(forbidden),
            "Hysteria2 inbound should use inbound-specific protocol datagram helpers instead of `{forbidden}`"
        );
    }
    assert!(
        inbound.contains("hysteria2::Hysteria2InboundUdpCodec")
            && protocol_udp.contains("struct Hysteria2InboundUdpCodec")
            && protocol_udp.contains("fn decode_datagram")
            && protocol_udp.contains("fn encode_datagram"),
        "Hysteria2 inbound should delegate UDP datagram framing through the protocol-owned inbound codec"
    );
}

#[test]
fn inbound_root_does_not_reexport_protocol_request_models() {
    let root = read("src/inbound/mod.rs");

    for forbidden in [
        "DirectInboundRequest",
        "Hysteria2InboundRequest",
        "MieruInboundRequest",
        "ShadowsocksInboundRequest",
        "TrojanInboundRequest",
        "ConfiguredVlessUser",
        "VlessInboundRequest",
        "VmessInboundRequest",
    ] {
        assert!(
            !root.contains(forbidden),
            "src/inbound/mod.rs should expose listener entrypoints, not protocol request model `{forbidden}`"
        );
    }
}

#[test]
fn protocol_inbound_roots_do_not_define_request_models() {
    for (root, model, request) in [
        (
            "src/inbound/vless/mod.rs",
            "src/inbound/vless/model.rs",
            "VlessInboundRequest",
        ),
        (
            "src/inbound/vmess/mod.rs",
            "src/inbound/vmess/model.rs",
            "VmessInboundRequest",
        ),
    ] {
        let root_content = read(root);
        let model_content = read(model);
        assert!(
            !root_content.contains(&format!("struct {request}")),
            "{root} should not define protocol request model `{request}`"
        );
        assert!(
            model_content.contains(&format!("struct {request}")),
            "{model} should own protocol request model `{request}`"
        );
    }
}

#[test]
fn vless_inbound_root_does_not_reexport_session_models() {
    let root = read("src/inbound/vless/mod.rs");
    let listener = read("src/inbound/vless/listener.rs");

    for forbidden in ["VlessStreamRequest", "VlessStreamTransport"] {
        assert!(
            !root.contains(forbidden),
            "src/inbound/vless/mod.rs should expose listener entrypoints, not session model `{forbidden}`"
        );
        assert!(
            listener.contains("use super::session::{VlessStreamRequest, VlessStreamTransport};"),
            "VLESS listener should import session models from the session module"
        );
    }
}

#[test]
fn adapter_roots_keep_udp_runtime_details_in_udp_modules() {
    let cases: &[(&str, &[&str])] = &[
        (
            "direct",
            &[
                "resolve_target_addr",
                "send_direct_packet",
                "UdpFlowOutbound::Direct",
                "resolve_udp_target",
                "udp_direct_send",
            ],
        ),
        (
            "hysteria2",
            &[
                "hysteria2_packet_path_carrier_descriptor",
                "build_hysteria2_packet_path",
                "Hysteria2DatagramSend",
                "UdpFlowOutbound::Hysteria2",
            ],
        ),
        (
            "mieru",
            &[
                "MieruDatagramSend",
                "MieruRelaySend",
                "send_mieru_",
                "UdpFlowOutbound::Mieru",
            ],
        ),
        (
            "shadowsocks",
            &[
                "shadowsocks_packet_path_carrier_descriptor",
                "build_shadowsocks_packet_path",
                "shadowsocks_udp_datagram_source",
                "ShadowsocksDatagramSend",
                "send_shadowsocks_datagram",
                "UdpFlowOutbound::Shadowsocks",
            ],
        ),
        (
            "socks5",
            &[
                "socks5_packet_path_carrier_descriptor",
                "build_socks5_packet_path",
                "Socks5UdpSend",
                "UdpFlowOutbound::Socks5",
            ],
        ),
        (
            "trojan",
            &[
                "TrojanDatagramSend",
                "TrojanRelaySend",
                "send_trojan_",
                "UdpFlowOutbound::Trojan",
            ],
        ),
        (
            "vless",
            &[
                "VlessDatagramSend",
                "VlessRelayFinalHopSend",
                "VlessRelayTwoStreamSend",
                "open_udp_stream",
                "encode_udp_packet",
                "dispatch_tcp_relay_prefix",
                "send_vless_",
            ],
        ),
        (
            "vmess",
            &["VmessDatagramSend", "VmessRelaySend", "send_vmess_"],
        ),
    ];

    for (adapter_name, forbidden_patterns) in cases {
        let adapter_path = format!("src/adapters/{adapter_name}.rs");
        let adapter = read(&adapter_path);
        let udp = manifest_dir().join(format!("src/adapters/{adapter_name}/udp.rs"));

        for forbidden in *forbidden_patterns {
            assert!(
                !adapter.contains(forbidden),
                "{adapter_path} should keep UDP runtime details in src/adapters/{adapter_name}/udp.rs; found `{forbidden}`"
            );
        }
        assert!(
            udp.exists(),
            "{adapter_name} adapter UDP runtime details should live in src/adapters/{adapter_name}/udp.rs"
        );
    }
}

#[test]
fn adapter_root_does_not_import_protocol_udp_request_types() {
    let adapters = read("src/adapters/mod.rs");

    for forbidden in [
        "ShadowsocksUdpFlow",
        "VlessUdpFlow",
        "VlessUdpRelayFinalHop",
        "VlessUdpRelayTwoStream",
        "VmessUdpFlow",
        "VmessUdpRelayFlow",
        "MieruUdpRelayFlow",
    ] {
        assert!(
            !adapters.contains(forbidden),
            "src/adapters/mod.rs should not globally import protocol UDP request type `{forbidden}`"
        );
    }
}

#[test]
fn adapter_root_is_facade_only() {
    let adapters = read("src/adapters/mod.rs");

    for expected in [
        "mod common;",
        "mod direct;",
        "mod http_connect;",
        "mod hysteria2;",
        "mod mieru;",
        "mod mixed;",
        "mod shadowsocks;",
        "mod socks5;",
        "mod trojan;",
        "mod vless;",
        "mod vmess;",
        "pub(crate) use direct::DirectAdapter;",
        "pub(crate) use http_connect::HttpConnectAdapter;",
        "pub(crate) use hysteria2::Hysteria2Adapter;",
        "pub(crate) use mieru::MieruAdapter;",
        "pub(crate) use mixed::MixedAdapter;",
        "pub(crate) use shadowsocks::ShadowsocksAdapter;",
        "pub(crate) use socks5::Socks5Adapter;",
        "pub(crate) use trojan::TrojanAdapter;",
        "pub(crate) use vless::VlessAdapter;",
        "pub(crate) use vmess::VmessAdapter;",
    ] {
        assert!(
            adapters.contains(expected),
            "src/adapters/mod.rs should expose adapter facade item `{expected}`"
        );
    }

    for line in adapters.lines().map(str::trim) {
        let allowed = line.is_empty()
            || line.starts_with("//!")
            || line.starts_with("#[cfg(")
            || line.starts_with("mod ")
            || line.starts_with("pub(crate) use ");
        assert!(
            allowed,
            "src/adapters/mod.rs should only declare adapter modules and re-export adapter types; found `{line}`"
        );
    }

    for forbidden in [
        "async fn",
        "fn ",
        "impl ",
        "match ",
        "InboundProtocolConfig::",
        "OutboundProtocolConfig::",
        "ResolvedLeafOutbound::",
        "ProtocolRegistry",
    ] {
        assert!(
            !adapters.contains(forbidden),
            "src/adapters/mod.rs should remain a facade over concrete adapter modules; found `{forbidden}`"
        );
    }
}

#[test]
fn adapter_roots_keep_tcp_runtime_details_in_tcp_modules() {
    let cases: &[(&str, &[&str])] = &[
        (
            "direct",
            &[
                ".direct_connector()\n            .connect(",
                "connect_direct",
                "EstablishedTcpOutbound::Direct",
            ],
        ),
        (
            "hysteria2",
            &[
                "crate::outbound::hysteria2::connect_tcp",
                "connect_upstream_hysteria2",
                "EstablishedTcpOutbound::Hysteria2",
            ],
        ),
        (
            "mieru",
            &[
                "crate::outbound::mieru::connect_tcp",
                "crate::outbound::mieru::apply_tcp_hop",
                "connect_upstream_mieru",
                "EstablishedTcpOutbound::Mieru",
            ],
        ),
        (
            "shadowsocks",
            &[
                "crate::outbound::shadowsocks::connect_tcp",
                "crate::outbound::shadowsocks::apply_tcp_hop",
                "connect_upstream_shadowsocks",
                "EstablishedTcpOutbound::Shadowsocks",
            ],
        ),
        (
            "socks5",
            &[
                "crate::outbound::socks5::connect_tcp",
                "crate::outbound::socks5::apply_tcp_hop",
                "connect_upstream_socks5",
                "EstablishedTcpOutbound::Socks5",
            ],
        ),
        (
            "trojan",
            &[
                "crate::outbound::trojan::connect_tcp",
                "crate::outbound::trojan::apply_tcp_hop",
                "connect_upstream_trojan",
                "EstablishedTcpOutbound::Trojan",
            ],
        ),
        (
            "vless",
            &[
                "crate::outbound::vless::connect_tcp",
                "crate::outbound::vless::apply_tcp_hop",
                "connect_upstream_vless",
                "EstablishedTcpOutbound::Vless",
            ],
        ),
        (
            "vmess",
            &[
                "crate::outbound::vmess::connect_tcp",
                "crate::outbound::vmess::apply_tcp_hop",
                "connect_upstream_vmess",
                "EstablishedTcpOutbound::Vmess",
            ],
        ),
    ];

    for (adapter_name, forbidden_patterns) in cases {
        let adapter_path = format!("src/adapters/{adapter_name}.rs");
        let adapter = read(&adapter_path);
        let tcp = manifest_dir().join(format!("src/adapters/{adapter_name}/tcp.rs"));

        for forbidden in *forbidden_patterns {
            assert!(
                !adapter.contains(forbidden),
                "{adapter_path} should keep TCP runtime details in src/adapters/{adapter_name}/tcp.rs; found `{forbidden}`"
            );
        }
        assert!(
            tcp.exists(),
            "{adapter_name} adapter TCP runtime details should live in src/adapters/{adapter_name}/tcp.rs"
        );
    }
}

#[test]
fn outbound_tcp_helpers_are_called_only_by_adapter_tcp_modules() {
    let helpers = [
        "crate::outbound::hysteria2::connect_tcp",
        "crate::outbound::mieru::connect_tcp",
        "crate::outbound::mieru::apply_tcp_hop",
        "crate::outbound::shadowsocks::connect_tcp",
        "crate::outbound::shadowsocks::apply_tcp_hop",
        "crate::outbound::socks5::connect_tcp",
        "crate::outbound::socks5::apply_tcp_hop",
        "crate::outbound::trojan::connect_tcp",
        "crate::outbound::trojan::apply_tcp_hop",
        "crate::outbound::vless::connect_tcp",
        "crate::outbound::vless::apply_tcp_hop",
        "crate::outbound::vmess::connect_tcp",
        "crate::outbound::vmess::apply_tcp_hop",
    ];

    for path in rust_sources_under("src") {
        let source = relative(&path);
        if source.starts_with("src/adapters/") && source.ends_with("/tcp.rs") {
            continue;
        }
        let content = fs::read_to_string(&path).expect("read rust source");
        for helper in helpers {
            assert!(
                !content.contains(helper),
                "{source} should not call outbound TCP helper `{helper}` directly; dispatch through the owning ProtocolAdapter"
            );
        }
    }
}

#[test]
fn trojan_tcp_connect_uses_request_model() {
    let outbound = read("src/outbound/trojan.rs");
    let adapter = read("src/adapters/trojan/tcp.rs");

    assert!(
        !outbound.contains("#[allow(clippy::too_many_arguments)]"),
        "Trojan TCP connect should not need a too_many_arguments allowance"
    );
    assert!(
        outbound.contains("struct TrojanTcpConnectRequest")
            && outbound.contains("request: TrojanTcpConnectRequest<'_>"),
        "Trojan TCP connect should use TrojanTcpConnectRequest"
    );
    assert!(
        adapter.contains("TrojanTcpConnectRequest {"),
        "Trojan adapter TCP module should pass TrojanTcpConnectRequest"
    );
}

#[test]
fn shadowsocks_tcp_connect_uses_request_model() {
    let outbound = read("src/outbound/shadowsocks.rs");
    let adapter = read("src/adapters/shadowsocks/tcp.rs");

    assert!(
        !outbound.contains("#[allow(clippy::too_many_arguments)]"),
        "Shadowsocks TCP connect should not need a too_many_arguments allowance"
    );
    assert!(
        outbound.contains("struct ShadowsocksTcpConnectRequest")
            && outbound.contains("request: ShadowsocksTcpConnectRequest<'_>"),
        "Shadowsocks TCP connect should use ShadowsocksTcpConnectRequest"
    );
    assert!(
        adapter.contains("ShadowsocksTcpConnectRequest {"),
        "Shadowsocks adapter TCP module should pass ShadowsocksTcpConnectRequest"
    );
    assert!(
        !outbound.contains("CipherKind::from_str"),
        "Shadowsocks outbound TCP helper should receive an adapter-parsed cipher"
    );
    assert!(
        adapter.contains("CipherKind::from_str"),
        "Shadowsocks adapter TCP module should own outbound cipher parsing"
    );
}

#[test]
fn vmess_tcp_connect_uses_request_model() {
    let outbound = read("src/outbound/vmess.rs");
    let adapter = read("src/adapters/vmess/tcp.rs");

    assert!(
        !outbound.contains("#[allow(clippy::too_many_arguments)]"),
        "VMess TCP connect should not need a too_many_arguments allowance"
    );
    assert!(
        outbound.contains("struct VmessTcpConnectRequest")
            && outbound.contains("request: VmessTcpConnectRequest<'_>"),
        "VMess TCP connect should use VmessTcpConnectRequest"
    );
    assert!(
        adapter.contains("VmessTcpConnectRequest {"),
        "VMess adapter TCP module should pass VmessTcpConnectRequest"
    );
    for forbidden in [
        "parse_uuid",
        "VmessCipher::from_name",
        "vmess unknown cipher",
        "VmessAeadStream::outbound",
        "TcpSessionProtocol",
        "VmessTcpSessionTarget",
    ] {
        assert!(
            !outbound.contains(forbidden),
            "VMess outbound TCP helper should receive adapter-parsed identity; found `{forbidden}`"
        );
    }
    for adapter_owned in [
        "parse_uuid",
        "VmessCipher::from_name",
        "vmess unknown cipher",
    ] {
        assert!(
            adapter.contains(adapter_owned),
            "VMess adapter TCP module should own outbound identity parsing detail `{adapter_owned}`"
        );
    }
    assert!(
        outbound.contains("vmess::establish_tcp_outbound_stream"),
        "VMess outbound TCP helper should delegate VMess session and AEAD setup to protocols/vmess"
    );
}

#[test]
fn vless_tcp_connect_uses_request_model() {
    let outbound = read("src/outbound/vless.rs");
    let adapter = read("src/adapters/vless/tcp.rs");

    assert!(
        !outbound.contains("#[allow(clippy::too_many_arguments)]"),
        "VLESS TCP connect should not need a too_many_arguments allowance"
    );
    assert!(
        outbound.contains("struct VlessTcpConnectRequest")
            && outbound.contains("request: VlessTcpConnectRequest<'_>"),
        "VLESS TCP connect should use VlessTcpConnectRequest"
    );
    assert!(
        adapter.contains("VlessTcpConnectRequest {"),
        "VLESS adapter TCP module should pass VlessTcpConnectRequest"
    );
    assert!(
        !outbound.contains("parse_uuid"),
        "VLESS outbound TCP helper should receive adapter-parsed identity"
    );
    assert!(
        adapter.contains("parse_uuid"),
        "VLESS adapter TCP module should own outbound identity parsing"
    );
}

#[test]
fn adapter_roots_keep_inbound_runtime_details_in_inbound_modules() {
    let cases: &[(&str, &[&str])] = &[
        (
            "direct",
            &["run_direct_listener_with_bound", "bound.into_tcp()"],
        ),
        (
            "http_connect",
            &["run_http_connect_listener_with_bound", "bound.into_tcp()"],
        ),
        (
            "hysteria2",
            &[
                "QuicInbound::bind",
                "run_hysteria2_listener_with_bound",
                "cert_path",
                "key_path",
            ],
        ),
        (
            "mieru",
            &["run_mieru_listener_with_bound", "bound.into_tcp()"],
        ),
        (
            "mixed",
            &["run_mixed_listener_with_bound", "bound.into_tcp()"],
        ),
        (
            "shadowsocks",
            &["run_shadowsocks_listener_with_bound", "bound.into_tcp()"],
        ),
        (
            "socks5",
            &["run_socks5_listener_with_bound", "bound.into_tcp()"],
        ),
        (
            "trojan",
            &["run_trojan_listener_with_bound", "bound.into_tcp()"],
        ),
        (
            "vless",
            &[
                "QuicInbound::bind",
                "zero_platform_tokio::TokioListener::bind",
                "run_vless_listener_with_bound",
                "quic.cert_path",
            ],
        ),
        (
            "vmess",
            &["run_vmess_listener_with_bound", "bound.into_tcp()"],
        ),
    ];

    for (adapter_name, forbidden_patterns) in cases {
        let adapter_path = format!("src/adapters/{adapter_name}.rs");
        let adapter = read(&adapter_path);
        let inbound = manifest_dir().join(format!("src/adapters/{adapter_name}/inbound.rs"));

        for forbidden in *forbidden_patterns {
            assert!(
                !adapter.contains(forbidden),
                "{adapter_path} should keep inbound runtime details in src/adapters/{adapter_name}/inbound.rs; found `{forbidden}`"
            );
        }
        assert!(
            inbound.exists(),
            "{adapter_name} adapter inbound runtime details should live in src/adapters/{adapter_name}/inbound.rs"
        );
    }
}

#[test]
fn adapter_modules_do_not_use_wildcard_parent_imports() {
    for path in rust_sources_under("src/adapters") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read adapter module");
        assert!(
            !content.contains("use super::*;"),
            "{source} should import its ProtocolAdapter dependencies explicitly"
        );
    }
}

#[test]
fn udp_dispatch_modules_do_not_use_wildcard_parent_imports() {
    for path in rust_sources_under("src/runtime/udp_dispatch") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read udp dispatch module");
        assert!(
            !content.contains("use super::*;"),
            "{source} should import UDP dispatch dependencies explicitly"
        );
    }
}

#[test]
fn protocol_inbound_submodules_do_not_use_wildcard_parent_imports() {
    for root in ["src/inbound/vless", "src/inbound/vmess"] {
        for path in rust_sources_under(root) {
            let source = relative(&path);
            let content = fs::read_to_string(&path).expect("read inbound protocol module");
            assert!(
                !content.contains("use super::*;"),
                "{source} should import protocol inbound dependencies explicitly"
            );
        }
    }
}

#[test]
fn protocol_named_inbound_modules_stay_proxy_glue_not_crypto_implementations() {
    for root in ["src/inbound/vless", "src/inbound/vmess"] {
        for path in rust_sources_under(root) {
            let source = relative(&path);
            let content = fs::read_to_string(&path).expect("read inbound protocol module");

            for forbidden in [
                "use aes",
                "use chacha",
                "use cipher",
                "use hmac",
                "use md5",
                "use ring",
                "use sha",
                "use uuid",
                "Aes128",
                "Aes256",
                "ChaCha20",
                "Hmac",
                "Md5",
                "Sha1",
                "Sha256",
                "Uuid::",
            ] {
                assert!(
                    !content.contains(forbidden),
                    "{source} should stay proxy-side inbound glue and delegate protocol crypto/parsing primitives to protocols/*; found `{forbidden}`"
                );
            }
        }
    }
}

#[test]
fn mieru_inbound_stream_uses_protocol_codec_not_crypto_primitives() {
    for path in rust_sources_under("src/inbound/mieru") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read mieru inbound module");

        for forbidden in [
            "build_data_segment",
            "parse_segment",
            "DataMetadata",
            "MieruCipher",
            "MieruSession",
            "DATA_SERVER_TO_CLIENT",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should wrap mieru protocol codecs instead of owning crypto/framing primitive `{forbidden}`"
            );
        }
    }

    let stream = read("src/inbound/mieru/model.rs");
    assert!(
        stream.contains("MieruInboundDataCodec")
            && stream.contains("decrypt_client_data_with_consumed")
            && stream.contains("encrypt_server_data"),
        "Mieru inbound stream adapter should delegate data-phase protocol logic to protocols/mieru"
    );
}

#[test]
fn shadowsocks_udp_inbound_uses_protocol_codec_not_datagram_primitives() {
    for path in rust_sources_under("src/inbound/shadowsocks") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read shadowsocks inbound module");

        for forbidden in [
            "ShadowsocksDatagramCodec",
            "decode_udp_datagram_2022_session",
            "encode_udp_response_2022",
            "ReplayWindow",
            "DatagramCodec",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should wrap Shadowsocks inbound UDP codecs instead of owning datagram primitive `{forbidden}`"
            );
        }
    }

    let udp = read("src/inbound/shadowsocks/udp.rs");
    assert!(
        udp.contains("ShadowsocksInboundUdpCodec")
            && udp.contains("decode_request")
            && udp.contains("encode_response"),
        "Shadowsocks inbound UDP should delegate protocol datagram logic to protocols/shadowsocks"
    );
}

#[test]
fn protocol_crates_do_not_depend_on_proxy_runtime_layers() {
    let protocols_root = repo_root().join("protocols");
    let forbidden = [
        "zero-proxy",
        "zero-api",
        "zero-config",
        "zero-router",
        "zero-engine",
        "zero-logging",
        "zero-dns",
        "zero-platform-tokio",
        "zero-transport",
        "zero-tun",
        "zero-stack",
    ];

    for entry in fs::read_dir(&protocols_root).expect("read protocols dir") {
        let entry = entry.expect("protocol entry");
        let manifest = entry.path().join("Cargo.toml");
        if !manifest.exists() {
            continue;
        }
        let content = fs::read_to_string(&manifest)
            .unwrap_or_else(|error| panic!("read {}: {error}", manifest.display()));
        for crate_name in forbidden {
            assert!(
                !content.contains(crate_name),
                "{} should not depend on forbidden runtime crate `{crate_name}`",
                manifest.display()
            );
        }
        for required in ["zero-core", "zero-traits"] {
            assert!(
                content.contains(required),
                "{} should stay anchored on protocol base crate `{required}`",
                manifest.display()
            );
        }
    }
}

#[test]
fn generic_udp_dispatch_does_not_encode_protocol_packets_directly() {
    let content = read("src/runtime/udp_dispatch/mod.rs");
    let dispatch = read("src/runtime/udp_dispatch/dispatch.rs");
    let lifecycle = read("src/runtime/udp_dispatch/lifecycle.rs");
    let types = read("src/runtime/udp_dispatch/types.rs");

    for forbidden in [
        "proxy.protocols.vless_outbound",
        "proxy.protocols.vmess_outbound",
        "VlessUdpPacketTarget",
        "VmessUdpPacketTarget",
        "VlessOutbound as UdpPacketFraming",
        "VmessOutbound as UdpPacketFraming",
    ] {
        assert!(
            !content.contains(forbidden),
            "src/runtime/udp_dispatch/mod.rs should stay protocol-neutral; found `{forbidden}`"
        );
    }

    for forbidden in ["VlessFlow", "VmessFlow", "vless_handles", "vmess_handles"] {
        for source in [&dispatch, &lifecycle, &types] {
            assert!(
                !source.contains(forbidden),
                "UDP dispatch should use neutral managed-flow state instead of `{forbidden}`"
            );
        }
    }
    assert!(
        types.contains("ManagedFlow") && dispatch.contains("managed_flows"),
        "UDP dispatch should track protocol-managed flows through neutral names"
    );
}

#[test]
fn protocol_inventory_keeps_protocol_instances_private() {
    let content = read("src/inventory.rs");
    let protocols = read("src/inventory/protocols.rs");

    for forbidden in [
        "InboundProtocolConfig::",
        "OutboundProtocolConfig::",
        "ResolvedLeafOutbound::",
        "pub socks5_inbound:",
        "pub socks5_outbound:",
        "pub http_connect_inbound:",
        "pub vless_inbound:",
        "pub vless_outbound:",
        "pub hysteria2_inbound:",
        "pub hysteria2_outbound:",
        "pub shadowsocks_inbound:",
        "pub shadowsocks_outbound:",
        "pub trojan_inbound:",
        "pub trojan_outbound:",
        "pub vmess_inbound:",
        "pub vmess_outbound:",
        "pub(crate) direct_outbound:",
    ] {
        assert!(
            !content.contains(forbidden),
            "src/inventory.rs should keep protocol instances private and delegate protocol classification to ProtocolRegistry; found `{forbidden}`"
        );
    }

    assert!(
        protocols.contains("fn direct_connector(&self)"),
        "src/inventory/protocols.rs should keep the neutral direct connector helper"
    );
}

#[test]
fn inventory_does_not_expose_concrete_protocol_accessors() {
    let protocol_access_patterns = [
        "use http_connect::",
        "use shadowsocks::",
        "use socks5::",
        "use trojan::",
        "use vless::",
        "use vmess::",
        "fn socks5_inbound_protocol(&self)",
        "fn socks5_outbound_protocol(&self)",
        "fn http_connect_inbound_protocol(&self)",
        "fn vless_inbound_protocol(&self)",
        "fn vless_outbound_protocol(&self)",
        "fn shadowsocks_outbound_protocol(&self)",
        "fn trojan_outbound_protocol(&self)",
        "fn vmess_outbound_protocol(&self)",
    ];

    for path in rust_sources_under("src/inventory") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        for forbidden in protocol_access_patterns {
            assert!(
                !content.contains(forbidden),
                "{source} should not import concrete protocol crates or expose concrete protocol accessors; found `{forbidden}`"
            );
        }
    }
}

#[test]
fn socks5_udp_association_runtime_state_stays_out_of_outbound_module() {
    let outbound = read("src/outbound/socks5.rs");
    let root = read("src/protocol_runtime/socks5_udp.rs");
    let active = read("src/protocol_runtime/socks5_udp/active.rs");
    let model = read("src/protocol_runtime/socks5_udp/model.rs");
    let packet_path_source = read("src/protocol_runtime/socks5_udp/packet_path.rs");
    let send_source = read("src/protocol_runtime/socks5_udp/send.rs");
    let send = manifest_dir().join("src/protocol_runtime/socks5_udp/send.rs");
    let runtime = manifest_dir().join("src/protocol_runtime/socks5_udp/runtime.rs");
    let packet_path = manifest_dir().join("src/protocol_runtime/socks5_udp/packet_path.rs");

    for forbidden in [
        "ActiveUpstreamSocks5UdpAssociation",
        "Socks5UdpAssociation",
        "UpstreamAssociationCloseReason",
        "send_socks5_udp_packet",
        "ensure_socks5_udp_association",
        "Socks5UdpRelay",
    ] {
        assert!(
            !outbound.contains(forbidden),
            "src/outbound/socks5.rs should stay focused on TCP handshake; found `{forbidden}`"
        );
    }

    for forbidden in [
        "Socks5UdpRelay",
        "ActiveUpstreamSocks5UdpAssociation",
        "UpstreamAssociationCloseReason",
        "Socks5UdpSend",
        "send_socks5_udp_packet",
        "ensure_socks5_udp_association",
    ] {
        assert!(
            !root.contains(forbidden),
            "src/protocol_runtime/socks5_udp.rs should stay a thin facade; found `{forbidden}`"
        );
    }

    assert!(
        active.contains("struct ActiveUpstreamSocks5UdpAssociation")
            && active.contains("Socks5UdpRelay"),
        "SOCKS5 UDP active association should live in protocol_runtime/socks5_udp/active.rs"
    );
    assert!(
        model.contains("enum UpstreamAssociationCloseReason")
            && model.contains("struct Socks5UdpAssociation"),
        "SOCKS5 UDP association model should live in protocol_runtime/socks5_udp/model.rs"
    );
    assert!(
        send_source.contains("async fn send_socks5_udp_packet")
            && send_source.contains("async fn ensure_socks5_udp_association"),
        "SOCKS5 UDP send orchestration should live in protocol_runtime/socks5_udp/send.rs"
    );
    assert!(
        !packet_path_source.contains("socks5::parse_udp_packet")
            && packet_path_source.contains("socks5::decode_udp_associate_response"),
        "SOCKS5 packet-path carrier should decode responses through semantic SOCKS5 associate helpers"
    );
    assert!(
        root.contains("Socks5UdpPacketSend")
            && !root.contains("pub(crate) use send::Socks5UdpSend"),
        "SOCKS5 UDP facade should expose only the packet-send facade model, not the internal send request"
    );
    assert!(
        send.exists() && runtime.exists() && packet_path.exists(),
        "SOCKS5 UDP runtime should be split into send.rs, runtime.rs, and packet_path.rs"
    );
}

#[test]
fn vless_udp_state_model_lives_outside_runtime_root() {
    let root = read("src/protocol_runtime/vless_udp.rs");
    let model = read("src/protocol_runtime/vless_udp/model.rs");

    for forbidden in [
        "struct VlessUdpUpstream",
        "struct VlessUdpTransport",
        "struct VlessUdpStartFlow",
        "struct VlessUdpRelayTwoStream",
        "struct VlessUdpRelayFinalHop",
        "struct VlessUdpUpstreamRequest",
    ] {
        assert!(
            !root.contains(forbidden),
            "vless_udp.rs should keep state/request models in vless_udp/model.rs; found `{forbidden}`"
        );
    }

    for required in [
        "struct VlessUdpUpstream",
        "struct VlessUdpTransport",
        "struct VlessUdpStartFlow",
        "struct VlessUdpRelayTwoStream",
        "struct VlessUdpRelayFinalHop",
        "struct VlessUdpUpstreamRequest",
    ] {
        assert!(
            model.contains(required),
            "VLESS UDP state/request model should live in vless_udp/model.rs; missing `{required}`"
        );
    }
}

#[test]
fn vless_udp_identity_is_adapter_parsed() {
    let runtime = read("src/protocol_runtime/vless_udp.rs");
    let model = read("src/protocol_runtime/vless_udp/model.rs");
    let flows = read("src/protocol_runtime/udp/flows.rs");
    let adapter = read("src/adapters/vless/udp.rs");

    assert!(
        !runtime.contains("parse_uuid"),
        "VLESS UDP runtime should receive adapter-parsed UUIDs"
    );
    assert!(
        !model.contains("id: &'a str") && model.contains("uuid: [u8; 16]"),
        "VLESS UDP request models should carry parsed UUIDs instead of raw config IDs"
    );
    let vless_flow_models = flows
        .split("pub(crate) struct VmessUdpFlow")
        .next()
        .expect("VLESS flow models should appear before VMess flow models");
    for forbidden in ["pub(crate) id: &'a str", "pub(super) id: &'a str"] {
        assert!(
            !vless_flow_models.contains(forbidden) && !model.contains(forbidden),
            "VLESS UDP request models should not carry raw config IDs; found `{forbidden}`"
        );
    }
    assert!(
        adapter.contains("parse_uuid"),
        "VLESS UDP adapter should own UUID parsing before calling protocol runtime"
    );
}

#[test]
fn vless_udp_adapter_delegates_packet_framing_to_protocol_helpers() {
    let adapter = read("src/adapters/vless/udp.rs");

    for forbidden in ["UdpPacketFraming", "VlessUdpPacketTarget"] {
        assert!(
            !adapter.contains(forbidden),
            "VLESS UDP adapter should delegate mux packet framing to protocols/vless helpers; found `{forbidden}`"
        );
    }
    assert!(
        !adapter.contains("vless::build_udp_packet")
            && !adapter.contains("vless::parse_udp_packet"),
        "VLESS UDP adapter should not call low-level packet helpers directly"
    );
    assert!(
        !adapter.contains("vless::encode_udp_flow_packet"),
        "VLESS UDP adapter should leave mux fast-path packet framing to protocol_runtime/vless_udp"
    );
}

#[test]
fn vless_udp_runtime_delegates_packet_framing_to_protocol_helpers() {
    let runtime = read("src/protocol_runtime/vless_udp.rs");

    for forbidden in [
        "UdpPacketFraming",
        "VlessUdpPacketTarget",
        "UdpPacketTunnelProtocol",
        "VlessUdpPacketTunnelTarget",
        "encode_udp_packet",
        "decode_udp_packet",
        "vless::build_udp_packet",
        "vless::parse_udp_packet",
        "vless::encode_udp_flow_packet",
        "vless::decode_udp_flow_packet",
    ] {
        assert!(
            !runtime.contains(forbidden),
            "VLESS UDP runtime should delegate packet framing to protocols/vless helpers; found `{forbidden}`"
        );
    }
    assert!(
        runtime.contains("vless::VlessUdpFlowCodec"),
        "VLESS UDP runtime should use the protocols/vless flow codec wrapper"
    );
    assert!(
        runtime.contains("vless::establish_udp_packet_tunnel"),
        "VLESS UDP runtime should call protocols/vless UDP tunnel helper"
    );
}

#[test]
fn vmess_udp_state_model_lives_outside_runtime_root() {
    let root = read("src/protocol_runtime/vmess_udp.rs");
    let model = read("src/protocol_runtime/vmess_udp/model.rs");

    for forbidden in [
        "struct VmessUdpUpstream",
        "struct VmessUdpTransport",
        "struct VmessUdpStartFlow",
        "struct VmessUdpRelayFlow",
        "struct VmessUdpUpstreamRequest",
    ] {
        assert!(
            !root.contains(forbidden),
            "vmess_udp.rs should keep state/request models in vmess_udp/model.rs; found `{forbidden}`"
        );
    }

    for required in [
        "struct VmessUdpUpstream",
        "struct VmessUdpTransport",
        "struct VmessUdpStartFlow",
        "struct VmessUdpRelayFlow",
        "struct VmessUdpUpstreamRequest",
    ] {
        assert!(
            model.contains(required),
            "VMess UDP state/request model should live in vmess_udp/model.rs; missing `{required}`"
        );
    }
}

#[test]
fn vmess_udp_identity_is_adapter_parsed() {
    let runtime = read("src/protocol_runtime/vmess_udp.rs");
    let model = read("src/protocol_runtime/vmess_udp/model.rs");
    let flows = read("src/protocol_runtime/udp/flows.rs");
    let adapter = read("src/adapters/vmess/udp.rs");

    for forbidden in ["parse_uuid", "VmessCipher::from_name"] {
        assert!(
            !runtime.contains(forbidden),
            "VMess UDP runtime should receive adapter-parsed identity; found `{forbidden}`"
        );
        assert!(
            adapter.contains(forbidden),
            "VMess UDP adapter should own identity parsing detail `{forbidden}`"
        );
    }

    let vmess_flow_models = flows
        .split("pub(crate) struct VmessUdpFlow")
        .nth(1)
        .expect("VMess UDP flow models should exist");
    for forbidden in [
        "pub(crate) id: &'a str",
        "pub(super) id: &'a str",
        "pub(crate) cipher: &'a str",
        "pub(super) cipher: &'a str",
    ] {
        assert!(
            !vmess_flow_models.contains(forbidden) && !model.contains(forbidden),
            "VMess UDP request models should carry parsed identity plus cipher_name only; found `{forbidden}`"
        );
    }
    assert!(
        model.contains("uuid: [u8; 16]")
            && model.contains("cipher_name: &'a str")
            && model.contains("cipher: vmess::VmessCipher"),
        "VMess UDP request models should carry parsed UUID/cipher plus cipher_name for mux"
    );
    assert!(
        model.contains("struct VmessUdpUpstreamRequest")
            && model.contains("pub(super) cipher_name: &'a str"),
        "VMess UDP upstream request should retain cipher_name for mux pool"
    );
}

#[test]
fn vmess_udp_runtime_delegates_packet_framing_to_protocol_helpers() {
    let runtime = read("src/protocol_runtime/vmess_udp.rs");

    for forbidden in [
        "UdpPacketFraming",
        "VmessUdpPacketTarget",
        "VmessAeadStream::establish_udp_outbound",
        "VmessOutbound",
        "encode_udp_packet",
        "decode_udp_packet",
        "vmess::build_udp_packet",
        "vmess::parse_udp_packet",
        "vmess::encode_udp_flow_packet",
        "vmess::decode_udp_flow_packet",
    ] {
        assert!(
            !runtime.contains(forbidden),
            "VMess UDP runtime should delegate packet framing to protocols/vmess helpers; found `{forbidden}`"
        );
    }
    assert!(
        runtime.contains("vmess::VmessUdpFlowCodec"),
        "VMess UDP runtime should use the protocols/vmess flow codec wrapper"
    );
    assert!(
        runtime.contains("vmess::establish_udp_outbound_stream"),
        "VMess UDP runtime should call protocols/vmess UDP stream helper"
    );
}

#[test]
fn vmess_mux_pool_model_lives_outside_runtime_root() {
    let root = read("src/protocol_runtime/vmess_mux_pool.rs");
    let model = read("src/protocol_runtime/vmess_mux_pool/model.rs");

    for forbidden in [
        "struct VmessMuxPoolKey",
        "enum VmessMuxTransportKey",
        "struct VmessMuxConn",
        "struct VmessMuxOpenRequest",
        "struct VmessMuxConnectionPool",
    ] {
        assert!(
            !root.contains(forbidden),
            "vmess_mux_pool.rs should keep pool/request models in vmess_mux_pool/model.rs; found `{forbidden}`"
        );
    }

    for required in [
        "struct VmessMuxPoolKey",
        "enum VmessMuxTransportKey",
        "struct VmessMuxConn",
        "struct VmessMuxOpenRequest",
        "struct VmessMuxConnectionPool",
    ] {
        assert!(
            model.contains(required),
            "VMess MUX pool model should live in vmess_mux_pool/model.rs; missing `{required}`"
        );
    }

    assert!(
        !root.contains("VmessMuxStream::new_with_network"),
        "VMess mux pool runtime should use the protocol mux stream helper instead of constructing VmessMuxStream directly"
    );
    assert!(
        root.contains("vmess::mux_stream_with_network"),
        "VMess mux pool runtime should call the protocol mux stream helper"
    );
    for forbidden in [
        "vmess::mux_cool_session",
        "vmess::VmessOutbound",
        "VmessAeadStream::outbound",
        "establish_tcp_session",
        "read_mux_frame_from_tokio",
    ] {
        assert!(
            !root.contains(forbidden),
            "VMess mux pool runtime should use the protocol mux connection helper instead of `{forbidden}`"
        );
    }
    assert!(
        root.contains("vmess::establish_mux_outbound_stream"),
        "VMess mux pool runtime should call the protocol mux connection helper"
    );
    assert!(
        root.contains("vmess::read_mux_stream_frame"),
        "VMess mux pool runtime should call the protocol mux frame reader helper"
    );
}

#[test]
fn vmess_mux_pool_receives_adapter_parsed_cipher() {
    let root = read("src/protocol_runtime/vmess_mux_pool.rs");
    let model = read("src/protocol_runtime/vmess_mux_pool/model.rs");
    let tcp_adapter = read("src/adapters/vmess/tcp.rs");
    let udp_adapter = read("src/adapters/vmess/udp.rs");

    assert!(
        !root.contains("VmessCipher::from_name"),
        "VMess mux pool should receive parsed cipher values from adapter-owned paths"
    );
    assert!(
        model.contains("cipher_name: String") && model.contains("cipher: vmess::VmessCipher"),
        "VMess mux pool request should carry cipher_name for keying and parsed VmessCipher for session setup"
    );
    assert!(
        tcp_adapter.contains("VmessCipher::from_name")
            && udp_adapter.contains("VmessCipher::from_name"),
        "VMess TCP/UDP adapters should own cipher parsing before mux pool use"
    );
}

#[test]
fn vless_mux_pool_model_lives_outside_runtime_root() {
    let root = read("src/protocol_runtime/vless_mux_pool.rs");
    let model = read("src/protocol_runtime/vless_mux_pool/model.rs");

    for forbidden in ["struct MuxConnectionPool", "struct VlessMuxOpenRequest"] {
        assert!(
            !root.contains(forbidden),
            "vless_mux_pool.rs should keep proxy-layer pool/request models in vless_mux_pool/model.rs; found `{forbidden}`"
        );
    }

    for required in ["struct MuxConnectionPool", "struct VlessMuxOpenRequest"] {
        assert!(
            model.contains(required),
            "VLESS MUX pool model should live in vless_mux_pool/model.rs; missing `{required}`"
        );
    }
    for forbidden in [
        "vless::encode_new_stream",
        "vless::encode_data_frame",
        "vless::encode_end_frame",
        "vless::MuxCrypto",
        "MuxCrypto::new",
    ] {
        assert!(
            !root.contains(forbidden),
            "VLESS mux pool runtime should use protocol mux_pool frame helpers instead of `{forbidden}`"
        );
    }
    for required in [
        "encode_mux_new_stream",
        "encode_mux_data_frame",
        "encode_mux_end_frame",
        "new_mux_crypto",
    ] {
        assert!(
            root.contains(required),
            "VLESS mux pool runtime should call protocol mux_pool helper `{required}`"
        );
    }
}

#[test]
fn protocol_runtime_udp_and_mux_roots_do_not_reexport_request_models() {
    for (source, forbidden) in [
        ("src/protocol_runtime/vless_udp.rs", "VlessUdpStartFlow"),
        (
            "src/protocol_runtime/vless_udp.rs",
            "VlessUdpRelayTwoStream",
        ),
        ("src/protocol_runtime/vless_udp.rs", "VlessUdpRelayFinalHop"),
        ("src/protocol_runtime/vless_udp.rs", "VlessUdpTransport"),
        ("src/protocol_runtime/vmess_udp.rs", "VmessUdpStartFlow"),
        ("src/protocol_runtime/vmess_udp.rs", "VmessUdpRelayFlow"),
        ("src/protocol_runtime/vmess_udp.rs", "VmessUdpTransport"),
        (
            "src/protocol_runtime/vless_mux_pool.rs",
            "VlessMuxOpenRequest",
        ),
        (
            "src/protocol_runtime/vmess_mux_pool.rs",
            "VmessMuxOpenRequest",
        ),
    ] {
        let content = read(source);
        assert!(
            !content.lines().any(
                |line| line.trim_start().starts_with("pub(crate) use model::")
                    && line.contains(forbidden)
            ),
            "{source} should not re-export request model `{forbidden}`"
        );
    }

    assert!(
        read("src/protocol_runtime/vless_mux_pool.rs")
            .contains("pub(crate) use model::MuxConnectionPool;"),
        "VLESS mux pool root should expose the pool type facade"
    );
    assert!(
        read("src/protocol_runtime/vmess_mux_pool.rs")
            .contains("pub(crate) use model::VmessMuxConnectionPool;"),
        "VMess mux pool root should expose the pool type facade"
    );
}

#[test]
fn inbound_vmess_mux_task_model_lives_outside_mux_root() {
    let root = read("src/inbound/vmess/mux.rs");
    let model = read("src/inbound/vmess/model.rs");

    for forbidden in [
        "struct VmessMuxTcpStreamTask",
        "struct VmessMuxUdpStreamTask",
    ] {
        assert!(
            !root.contains(forbidden),
            "inbound/vmess/mux.rs should keep MUX task models in inbound/vmess/model.rs; found `{forbidden}`"
        );
    }

    for required in [
        "struct VmessMuxTcpStreamTask",
        "struct VmessMuxUdpStreamTask",
    ] {
        assert!(
            model.contains(required),
            "VMess inbound MUX task model should live in inbound/vmess/model.rs; missing `{required}`"
        );
    }
    assert!(
        !root.contains("read_mux_frame_from_tokio"),
        "VMess inbound MUX runtime should use the protocol mux frame reader helper"
    );
    assert!(
        root.contains("vmess::read_mux_stream_frame"),
        "VMess inbound MUX runtime should call the protocol mux frame reader helper"
    );
}

#[test]
fn vmess_inbound_udp_response_encoding_stays_in_protocol_crate() {
    let helper = read("src/inbound/vmess/helpers.rs");
    let mux = read("src/inbound/vmess/mux.rs");
    let protocol_udp = manifest_dir()
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .join("protocols/vmess/src/udp.rs");
    let protocol_udp = fs::read_to_string(protocol_udp).expect("read vmess protocol udp source");

    assert!(
        !helper.contains("vmess::build_udp_packet"),
        "VMess inbound helper should not build protocol UDP response packets directly"
    );
    assert!(
        !mux.contains("vmess::parse_udp_packet"),
        "VMess inbound MUX/session glue should delegate VMess UDP request parsing to protocols/vmess"
    );
    assert!(
        !mux.contains("socks5::parse_udp_packet")
            && mux.contains("socks5::decode_udp_associate_response"),
        "VMess inbound SOCKS5 upstream response bridge should use semantic SOCKS5 associate helpers"
    );
    for forbidden in [
        "vmess::encode_udp_response",
        "vmess::encode_mux_udp_response",
        "vmess::decode_inbound_udp_payload",
        "vmess::encode_inbound_udp_response",
        "vmess::encode_inbound_mux_udp_response",
        "vmess::decode_inbound_udp_datagram",
    ] {
        assert!(
            !helper.contains(forbidden),
            "VMess inbound helper should use inbound-specific protocol helpers; found `{forbidden}`"
        );
    }
    assert!(
        helper.contains("vmess::VmessInboundUdpCodec")
            && protocol_udp.contains("struct VmessInboundUdpCodec")
            && protocol_udp.contains("fn encode_response")
            && protocol_udp.contains("fn encode_mux_response")
            && protocol_udp.contains("fn decode_datagram"),
        "VMess inbound UDP packet framing should go through the protocols/vmess inbound codec wrapper"
    );
}

#[test]
fn inbound_vless_mux_task_model_lives_outside_mux_root() {
    let root = read("src/inbound/vless/mux.rs");
    let model = read("src/inbound/vless/model.rs");

    assert!(
        !root.contains("struct VlessMuxUdpStreamTask"),
        "inbound/vless/mux.rs should keep MUX task models in inbound/vless/model.rs"
    );

    assert!(
        model.contains("struct VlessMuxUdpStreamTask"),
        "VLESS inbound MUX task model should live in inbound/vless/model.rs"
    );
}

#[test]
fn vless_inbound_udp_packet_framing_stays_in_protocol_crate() {
    let helper = read("src/inbound/vless/helpers.rs");
    let udp_session = read("src/inbound/vless/udp_session.rs");
    let mux = read("src/inbound/vless/mux.rs");
    let protocol_shared = manifest_dir()
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .join("protocols/vless/src/shared.rs");
    let protocol_shared =
        fs::read_to_string(protocol_shared).expect("read vless protocol shared source");

    for (source_name, source) in [
        ("inbound/vless/helpers.rs", helper.as_str()),
        ("inbound/vless/udp_session.rs", udp_session.as_str()),
        ("inbound/vless/mux.rs", mux.as_str()),
    ] {
        for forbidden in ["vless::build_udp_packet", "vless::parse_udp_packet"] {
            assert!(
                !source.contains(forbidden),
                "{source_name} should delegate VLESS UDP packet framing to protocols/vless; found `{forbidden}`"
            );
        }
    }
    for (source_name, source) in [
        ("inbound/vless/udp_session.rs", udp_session.as_str()),
        ("inbound/vless/mux.rs", mux.as_str()),
    ] {
        assert!(
            !source.contains("socks5::parse_udp_packet")
                && source.contains("socks5::decode_udp_associate_response"),
            "{source_name} should use semantic SOCKS5 associate helpers for upstream response bridging"
        );
    }

    for forbidden in [
        "vless::decode_inbound_udp_packet",
        "vless::encode_udp_response",
        "vless::encode_mux_udp_response",
        "vless::decode_inbound_udp_datagram",
        "vless::encode_inbound_udp_response",
        "vless::encode_inbound_mux_udp_response",
    ] {
        assert!(
            !helper.contains(forbidden),
            "VLESS inbound helper should use inbound-specific protocol helpers; found `{forbidden}`"
        );
    }
    assert!(
        helper.contains("vless::VlessInboundUdpCodec")
            && protocol_shared.contains("struct VlessInboundUdpCodec")
            && protocol_shared.contains("fn decode_datagram")
            && protocol_shared.contains("fn encode_response")
            && protocol_shared.contains("fn encode_mux_response"),
        "VLESS inbound UDP packet framing should go through the protocols/vless inbound codec wrapper"
    );
}

#[test]
fn trojan_inbound_udp_packet_framing_stays_in_protocol_crate() {
    let inbound = read("src/inbound/trojan.rs");
    let protocol_outbound = manifest_dir()
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .join("protocols/trojan/src/outbound.rs");
    let protocol_outbound =
        fs::read_to_string(protocol_outbound).expect("read trojan protocol outbound source");

    for forbidden in [
        "TrojanUdpPacket {",
        "UdpPacketStreamFraming<TrojanUdpPacket>",
        "TrojanOutbound as UdpPacketStreamFraming",
        "trojan::read_inbound_udp_packet",
        "trojan::write_udp_response",
        "socks5::parse_udp_packet",
    ] {
        assert!(
            !inbound.contains(forbidden),
            "inbound/trojan.rs should delegate Trojan UDP packet framing to protocols/trojan; found `{forbidden}`"
        );
    }
    assert!(
        inbound.contains("socks5::decode_udp_associate_response"),
        "Trojan inbound SOCKS5 upstream response bridge should use semantic SOCKS5 associate helpers"
    );

    for required in ["read_udp_flow_packet", "write_udp_flow_packet"] {
        assert!(
            protocol_outbound.contains(required)
                && inbound.contains(&format!("trojan::{required}")),
            "Trojan inbound UDP packet framing should be owned by protocols/trojan `{required}`"
        );
    }
}

#[test]
fn mieru_client_stream_model_lives_outside_inbound_root() {
    let root = read("src/inbound/mieru.rs");
    let model = read("src/inbound/mieru/model.rs");

    for forbidden in [
        "struct MieruClientStream",
        "impl AsyncRead for MieruClientStream",
        "impl AsyncWrite for MieruClientStream",
    ] {
        assert!(
            !root.contains(forbidden),
            "inbound/mieru.rs should keep client stream state in inbound/mieru/model.rs; found `{forbidden}`"
        );
    }

    for required in [
        "struct MieruClientStream",
        "impl AsyncRead for MieruClientStream",
        "impl AsyncWrite for MieruClientStream",
    ] {
        assert!(
            model.contains(required),
            "Mieru client stream state should live in inbound/mieru/model.rs; missing `{required}`"
        );
    }
}

#[test]
fn mieru_inbound_udp_packet_framing_stays_in_protocol_crate() {
    let inbound = read("src/inbound/mieru.rs");
    let protocol_udp = manifest_dir()
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .join("protocols/mieru/src/udp.rs");
    let protocol_udp = fs::read_to_string(protocol_udp).expect("read mieru protocol udp source");

    for forbidden in [
        "mieru::unwrap_udp_associate",
        "mieru::wrap_udp_associate",
        "mieru::decode_inbound_udp_packet",
        "mieru::encode_udp_response",
        "mieru::decode_udp_flow_packet",
        "mieru::encode_udp_flow_packet",
        "socks5::parse_udp_packet",
        "socks5::build_udp_packet",
    ] {
        assert!(
            !inbound.contains(forbidden),
            "inbound/mieru.rs should delegate Mieru UDP packet framing to protocols/mieru; found `{forbidden}`"
        );
    }

    assert!(
        inbound.contains("mieru::MieruUdpFlowCodec")
            && protocol_udp.contains("struct MieruUdpFlowCodec")
            && protocol_udp.contains("fn encode_packet")
            && protocol_udp.contains("fn decode_packet"),
        "Mieru inbound UDP packet framing should go through the protocols/mieru flow codec wrapper"
    );
}

#[test]
fn socks5_udp_send_details_stay_out_of_udp_dispatch() {
    let dispatch = read("src/runtime/udp_dispatch/socks5_flow.rs");
    let forward = read("src/runtime/udp_dispatch/forward.rs");
    let socks5_adapter = read("src/adapters/socks5/udp.rs");

    for forbidden in [
        "Socks5UdpAssociation {",
        "send_socks5_udp_packet",
        "UpstreamAssociationCloseReason::Dropped",
        "log_udp_upstream_association_dropped",
        "record_udp_upstream_send_failure",
    ] {
        assert!(
            !dispatch.contains(forbidden),
            "runtime UDP dispatch should delegate SOCKS5 UDP send details to protocol_runtime; found `{forbidden}`"
        );
    }
    for source in [&forward, &socks5_adapter] {
        assert!(
            !source.contains("Socks5UdpSend"),
            "UDP forward/adapters should call UdpDispatch::send_socks5 without constructing protocol-runtime request models"
        );
    }
    assert!(
        dispatch.contains("crate::protocol_runtime::socks5_udp::Socks5UdpPacketSend")
            && dispatch.contains("pub(crate) async fn send_socks5(")
            && dispatch.contains("resume: ProtocolUdpFlowResume")
            && !dispatch.contains("username: Option<&'a str>")
            && !dispatch.contains("password: Option<&'a str>")
            && !forward.contains("socks5_relay_auth")
            && !forward.contains("username: auth.username")
            && !forward.contains("password: auth.password"),
        "runtime UDP SOCKS5 facade should construct the protocol-runtime facade request"
    );
}

#[test]
fn socks5_udp_association_close_details_stay_out_of_udp_associate_loop() {
    let associate = read("src/protocol_runtime/socks5_udp_associate.rs");

    for forbidden in [
        "UpstreamAssociationCloseReason",
        ".close(",
        ".outbound_tag()",
        ".upstream_endpoint()",
        ".take_socks5_upstream()",
        ".socks5_upstream()",
    ] {
        assert!(
            !associate.contains(forbidden),
            "SOCKS5 UDP associate loop should use dispatch/runtime facades instead of association internals; found `{forbidden}`"
        );
    }
}

#[test]
fn socks5_udp_associate_loop_delegates_dispatch_and_direct_response_framing() {
    let associate = read("src/protocol_runtime/socks5_udp_associate.rs");
    let chain_response = read("src/protocol_runtime/socks5_udp_associate/chain_response.rs");
    let cleanup = read("src/protocol_runtime/socks5_udp_associate/cleanup.rs");
    let dispatch = read("src/protocol_runtime/socks5_udp_associate/dispatch.rs");
    let direct_response = read("src/protocol_runtime/socks5_udp_associate/direct_response.rs");
    let idle_timeout = read("src/protocol_runtime/socks5_udp_associate/idle_timeout.rs");
    let relay_socket = read("src/protocol_runtime/socks5_udp_associate/relay_socket.rs");
    let setup = read("src/protocol_runtime/socks5_udp_associate/setup.rs");
    let upstream_response = read("src/protocol_runtime/socks5_udp_associate/upstream_response.rs");

    for forbidden in [
        "UdpPipeInput",
        "ProtocolType::Socks5",
        "DnsResolver",
        ".resolver.resolve(",
        "async fn dispatch_packet",
        "async fn forward_direct_udp_response",
        "async fn forward_chain_response",
        "socks5::encode_udp_associate_response(&address_from_socket_addr",
        "direct_response_session_id",
        "record_session_outbound_rx",
        "record_session_inbound_tx",
        "failed to forward direct UDP response",
        "socks5::encode_udp_associate_response(target",
        "failed to send UDP chain response to client",
        "failed to build SOCKS5 UDP chain response",
        "chain upstream read error",
        "chain response task panicked",
        "async fn handle_upstream_response",
        "socks5_upstream_view",
        "upstream_response_session_id",
        "record_udp_upstream_recv_failure",
        "log_udp_upstream_association_dropped",
        "failed to attribute upstream UDP response",
        "async fn handle_idle_timeout",
        "fn handle_idle_timeout",
        "drop_socks5_idle",
        "log_udp_upstream_association_idle_timeout",
        "async fn handle_relay_packet",
        "client_udp_addr.is_none",
        "failed to process UDP packet",
        "dropping udp packet from unexpected sender",
        "Socks5Reply::Succeeded",
        "send_response_with_bound",
        "bind_addr(SocketAddr::new",
        "socks5 udp association ready",
        "drain_traffic",
        "finish_all",
        "log_completed_udp_flow",
    ] {
        assert!(
            !associate.contains(forbidden),
            "SOCKS5 UDP associate loop should delegate dispatch/direct response details; found `{forbidden}`"
        );
    }

    assert!(
        dispatch.contains("async fn dispatch_packet")
            && dispatch.contains("UdpPipeInput")
            && dispatch.contains("ProtocolType::Socks5")
            && dispatch.contains(".resolver.resolve("),
        "SOCKS5 UDP packet dispatch should live in socks5_udp_associate/dispatch.rs"
    );
    assert!(
        direct_response.contains("async fn forward_direct_udp_response")
            && direct_response.contains("async fn forward_relay_socket_response")
            && direct_response.contains("async fn forward_dispatch_socket_response")
            && direct_response.contains("direct_response_session_id")
            && direct_response.contains("socks5::encode_udp_associate_response"),
        "SOCKS5 UDP direct response metering and framing should live in socks5_udp_associate/direct_response.rs"
    );
    assert!(
        chain_response.contains("async fn handle_chain_result")
            && chain_response.contains("pub(super) struct ChainResponseRequest")
            && chain_response.contains("struct ForwardChainResponseRequest")
            && chain_response.contains("socks5::encode_udp_associate_response(request.target")
            && chain_response.contains("failed to send UDP chain response to client")
            && chain_response.contains("chain response task panicked"),
        "SOCKS5 UDP chain response result handling and framing should live in socks5_udp_associate/chain_response.rs"
    );
    for (path, source) in [
        ("dispatch.rs", &dispatch),
        ("direct_response.rs", &direct_response),
        ("chain_response.rs", &chain_response),
        ("upstream_response.rs", &upstream_response),
    ] {
        for forbidden in ["socks5::parse_udp_packet", "socks5::build_udp_packet"] {
            assert!(
                !source.contains(forbidden),
                "SOCKS5 UDP associate {path} should use semantic associate packet helpers instead of `{forbidden}`"
            );
        }
    }
    assert!(
        dispatch.contains("socks5::decode_udp_associate_request")
            && upstream_response.contains("socks5::decode_udp_associate_response"),
        "SOCKS5 UDP associate dispatch/attribution should use semantic decode helpers"
    );
    assert!(
        upstream_response.contains("async fn handle_upstream_response")
            && upstream_response.contains("socks5_upstream_view")
            && upstream_response.contains("upstream_response_session_id")
            && upstream_response.contains("record_udp_upstream_recv_failure")
            && upstream_response.contains("failed to attribute upstream UDP response"),
        "SOCKS5 UDP upstream response attribution and cleanup should live in socks5_udp_associate/upstream_response.rs"
    );
    assert!(
        idle_timeout.contains("fn handle_idle_timeout")
            && idle_timeout.contains("drop_socks5_idle")
            && idle_timeout.contains("log_udp_upstream_association_idle_timeout"),
        "SOCKS5 UDP idle timeout cleanup should live in socks5_udp_associate/idle_timeout.rs"
    );
    assert!(
        relay_socket.contains("async fn handle_relay_packet")
            && relay_socket.contains("pub(super) struct RelayPacketRequest")
            && relay_socket.contains("client_udp_addr.is_none")
            && relay_socket.contains("failed to process UDP packet")
            && relay_socket.contains("dropping udp packet from unexpected sender"),
        "SOCKS5 UDP relay socket packet classification should live in socks5_udp_associate/relay_socket.rs"
    );
    assert!(
        setup.contains("async fn setup_association")
            && setup.contains("Socks5Reply::Succeeded")
            && setup.contains("send_response_with_bound")
            && setup.contains("bind_addr(SocketAddr::new")
            && setup.contains("socks5 udp association ready")
            && setup.contains("drain_traffic"),
        "SOCKS5 UDP associate bind/response setup should live in socks5_udp_associate/setup.rs"
    );
    assert!(
        cleanup.contains("fn finish_dispatch")
            && cleanup.contains("finish_all")
            && cleanup.contains("log_completed_udp_flow"),
        "SOCKS5 UDP associate cleanup should live in socks5_udp_associate/cleanup.rs"
    );
}

#[test]
fn udp_dispatch_poll_refs_does_not_expose_socks5_association_type() {
    let lifecycle = read("src/runtime/udp_dispatch/lifecycle.rs");

    for forbidden in [
        "Option<&crate::protocol_runtime::socks5_udp::ActiveUpstreamSocks5UdpAssociation>",
        "self.socks5.upstream()",
    ] {
        assert!(
            !lifecycle.contains(forbidden),
            "UdpDispatch poll refs should expose Socks5UdpRuntime facade, not SOCKS5 association internals; found `{forbidden}`"
        );
    }
    assert!(
        lifecycle.contains("Socks5UdpRuntime")
            && lifecycle.contains("Socks5UdpAssociationView")
            && lifecycle.contains("ClosedSocks5UdpAssociation"),
        "UdpDispatch lifecycle should expose SOCKS5 facade types through local imports"
    );
    assert!(
        !lifecycle.contains("crate::protocol_runtime::socks5_udp::Socks5UdpRuntime")
            && !lifecycle.contains("crate::protocol_runtime::socks5_udp::Socks5UdpAssociationView")
            && !lifecycle
                .contains("crate::protocol_runtime::socks5_udp::ClosedSocks5UdpAssociation"),
        "UdpDispatch lifecycle should not scatter fully-qualified SOCKS5 runtime facade type paths"
    );
}

#[test]
fn generic_runtime_root_does_not_import_protocol_crates_directly() {
    for path in rust_sources_under("src/runtime") {
        let source = relative(&path);

        let content = fs::read_to_string(&path).expect("read rust source");
        for protocol_crate in [
            "use socks5::",
            "use vless::",
            "use vmess::",
            "use shadowsocks::",
            "use trojan::",
            "use hysteria2::",
            "use mieru::",
        ] {
            assert!(
                !content.contains(protocol_crate),
                "{source} should not import protocol crate `{protocol_crate}` directly; move protocol state to src/protocol_runtime"
            );
        }
    }
}

#[test]
fn generic_runtime_udp_state_uses_protocol_neutral_module_name() {
    let runtime_root = manifest_dir().join("src/runtime");

    for forbidden in ["udp_associate.rs", "udp_associate"] {
        let path = runtime_root.join(forbidden);
        assert!(
            !path.exists(),
            "src/runtime/{forbidden} should stay protocol-neutral as src/runtime/udp_flow"
        );
    }
}

#[test]
fn udp_flow_helpers_do_not_depend_on_protocol_runtime() {
    let content = read("src/runtime/udp_flow/helpers.rs");

    assert!(
        !content.contains("protocol_runtime::"),
        "src/runtime/udp_flow/helpers.rs should stay protocol-neutral"
    );
}

#[test]
fn udp_packet_path_carrier_snapshot_is_protocol_neutral() {
    let runtime = read("src/runtime/udp_flow/sessions.rs");
    let protocol_runtime = read("src/protocol_runtime/udp/packet_path_snapshot.rs");
    let traits = read("src/protocol_runtime/udp/packet_path_traits/carrier.rs");

    assert!(
        !runtime.contains("enum UdpPacketPathCarrier"),
        "protocol-named packet-path carrier snapshots should not be declared in generic runtime UDP flow state"
    );
    assert!(
        !protocol_runtime.contains("enum UdpPacketPathCarrier"),
        "packet-path carrier snapshot storage should not remain a protocol-named enum"
    );
    assert!(
        !traits.contains("struct PacketPathCarrierSnapshot")
            && traits.contains("struct PacketPathCarrierDescriptor")
            && traits.contains("carrier_cache_key: String"),
        "packet-path flow snapshots should derive carrier identity directly from the adapter-built carrier descriptor"
    );
    assert!(
        traits.contains("struct PacketPathFlowSnapshot")
            && traits.contains("carrier_cache_key: String")
            && traits.contains("datagram_cache_key: String"),
        "packet-path flow snapshots should store only neutral carrier/datagram cache identities"
    );
}

#[test]
fn udp_flow_outbound_snapshot_is_not_declared_in_session_bookkeeping() {
    let sessions = read("src/runtime/udp_flow/sessions.rs");
    let outbound = read("src/runtime/udp_flow/outbound.rs");

    assert!(
        !sessions.contains("enum UdpFlowOutbound"),
        "UdpFlowOutbound should not be declared in generic UDP session bookkeeping"
    );
    assert!(
        outbound.contains("enum UdpFlowOutbound"),
        "runtime::udp_flow::outbound should own UdpFlowOutbound"
    );
}

#[test]
fn udp_flow_outbound_snapshot_uses_neutral_runtime_variants() {
    let outbound = read("src/runtime/udp_flow/outbound.rs");
    let snapshot = read("src/protocol_runtime/udp/flow_snapshot.rs");

    for required in [
        "Direct {",
        "Relay {",
        "Datagram {",
        "StreamPacket {",
        "PacketPathDatagram {",
        "ProtocolUdpFlowSnapshot",
    ] {
        assert!(
            outbound.contains(required),
            "runtime UDP outbound snapshot should expose neutral variant or opaque snapshot `{required}`"
        );
    }

    for forbidden in [
        "Socks5 {",
        "Shadowsocks {",
        "Hysteria2 {",
        "Trojan {",
        "Mieru {",
        "username: Option<String>",
        "password: Option<String>",
        "UdpPacketPathCarrier::",
    ] {
        assert!(
            !outbound.contains(forbidden),
            "runtime UDP outbound snapshot should not declare protocol detail `{forbidden}`"
        );
    }
    let snapshot_enum = snapshot
        .split("pub(crate) enum ProtocolUdpFlowSnapshot")
        .nth(1)
        .expect("ProtocolUdpFlowSnapshot enum should exist")
        .split("#[derive(Debug, Clone, PartialEq, Eq)]")
        .next()
        .expect("ProtocolUdpFlowResume should follow ProtocolUdpFlowSnapshot");
    assert!(
        snapshot_enum.contains("Managed {")
            && snapshot_enum.contains("resume: ProtocolUdpFlowResume")
            && !snapshot_enum.contains("Socks5")
            && !snapshot_enum.contains("Shadowsocks")
            && !snapshot_enum.contains("Hysteria2")
            && !snapshot_enum.contains("Trojan")
            && !snapshot_enum.contains("Mieru"),
        "protocol UDP flow snapshot should expose only the unified managed resume wrapper"
    );
    let resume_enum = snapshot
        .split("pub(crate) enum ProtocolUdpFlowResume")
        .nth(1)
        .expect("ProtocolUdpFlowResume enum should exist")
        .split("impl ProtocolUdpFlowSnapshot")
        .next()
        .expect("ProtocolUdpFlowSnapshot impl should follow ProtocolUdpFlowResume");
    assert!(
        resume_enum.contains("Socks5(socks5::Socks5UdpFlowResume)")
            && resume_enum.contains("Shadowsocks(shadowsocks::ShadowsocksUdpFlowResume)")
            && resume_enum.contains("Hysteria2(hysteria2::Hysteria2UdpFlowResume)")
            && resume_enum.contains("Trojan(trojan::TrojanUdpFlowResume)")
            && resume_enum.contains("Mieru(mieru::MieruUdpFlowResume)")
            && !resume_enum.contains("username: Option<String>")
            && !resume_enum.contains("password: String")
            && !resume_enum.contains("password: Option<String>")
            && !resume_enum.contains("client_fingerprint: Option<String>")
            && !resume_enum.contains("relay_chain: bool")
            && !resume_enum.contains("cipher_kind: shadowsocks::CipherKind"),
        "ProtocolUdpFlowResume should wrap protocol-owned resume objects without exposing protocol-private fields"
    );
    assert!(
        !snapshot.contains("PacketPathCarrierSnapshot")
            && !snapshot.contains("UdpPacketPathCarrier::"),
        "protocol UDP flow snapshot should not own packet-path carrier identity"
    );
    assert!(
        outbound.contains("snapshot: crate::protocol_runtime::udp::PacketPathFlowSnapshot"),
        "runtime UDP outbound snapshot should keep packet-path flow identity in a neutral packet-path snapshot"
    );
}

#[test]
fn udp_session_bookkeeping_does_not_match_protocol_outbound_variants() {
    let content = read("src/runtime/udp_flow/sessions.rs");
    let outbound = read("src/runtime/udp_flow/outbound.rs");

    for forbidden in [
        "UdpFlowOutbound::Shadowsocks",
        "UdpFlowOutbound::Hysteria2",
        "UdpFlowOutbound::Trojan",
        "UdpFlowOutbound::Mieru",
    ] {
        assert!(
            !content.contains(forbidden),
            "src/runtime/udp_flow/sessions.rs should not match protocol UDP outbound variant `{forbidden}`"
        );
    }
    for forbidden in [
        ".direct_sender()",
        ".upstream_response_tag()",
        ".matches_upstream_tag(",
        ".upstream_endpoint()",
        ".success_outcome()",
    ] {
        assert!(
            !content.contains(forbidden),
            "src/runtime/udp_flow/sessions.rs should consume outbound index/completion views instead of fine-grained outbound accessors; found `{forbidden}`"
        );
    }
    assert!(
        content.contains(".index_keys()") && content.contains(".completion()"),
        "src/runtime/udp_flow/sessions.rs should use UdpFlowOutbound index/completion views"
    );
    assert!(
        outbound.contains("struct UdpFlowIndexKeys")
            && outbound.contains("struct UdpFlowCompletion")
            && outbound.contains("pub(super) fn index_keys(")
            && outbound.contains("pub(super) fn completion("),
        "src/runtime/udp_flow/outbound.rs should own UDP flow index/completion view derivation"
    );
}

#[test]
fn generic_udp_dispatch_does_not_contain_protocol_manager_modules() {
    let forbidden = [
        "protocol_flows.rs",
        "h2_manager.rs",
        "mieru_manager.rs",
        "packet_path_chain.rs",
        "packet_path_traits.rs",
        "ss_manager.rs",
        "trojan_manager.rs",
    ];

    for file_name in forbidden {
        let path = manifest_dir()
            .join("src/runtime/udp_dispatch")
            .join(file_name);
        assert!(
            !path.exists(),
            "src/runtime/udp_dispatch/{file_name} should live under src/protocol_runtime/udp"
        );
    }
}

#[test]
fn udp_dispatch_keeps_protocol_managers_in_protocol_runtime_state() {
    let content = read("src/runtime/udp_dispatch/mod.rs");
    let state = read("src/protocol_runtime/udp/state.rs");

    assert!(
        content.contains("protocol_state: ProtocolUdpState"),
        "UdpDispatch should keep protocol-specific managers behind ProtocolUdpState"
    );
    assert!(
        !content.contains("socks5: Socks5UdpRuntime"),
        "UdpDispatch should keep SOCKS5 UDP association state inside ProtocolUdpState"
    );
    assert!(
        state.contains("socks5: Socks5UdpRuntime"),
        "ProtocolUdpState should own the SOCKS5 UDP association facade"
    );

    for forbidden in [
        "socks5_upstream:",
        "socks5_idle_deadline:",
        "ActiveUpstreamSocks5UdpAssociation",
        "vless_manager:",
        "vmess_manager:",
        "ss_manager:",
        "packet_path_manager:",
        "trojan_manager:",
        "mieru_manager:",
        "h2_manager:",
    ] {
        assert!(
            !content.contains(forbidden),
            "UdpDispatch should not declare protocol manager field `{forbidden}` directly"
        );
    }
}

#[test]
fn udp_dispatch_internal_state_fields_are_not_crate_public() {
    let content = read("src/runtime/udp_dispatch/mod.rs");

    for field in [
        "inbound_tag",
        "flows",
        "direct_socket",
        "socks5",
        "protocol_state",
        "chain_tasks",
    ] {
        assert!(
            !content.contains(&format!("pub(crate) {field}:")),
            "UdpDispatch field `{field}` should stay private behind methods"
        );
    }
}

#[test]
fn protocol_udp_flow_requests_do_not_extend_udp_dispatch() {
    let content = read("src/protocol_runtime/udp/flows.rs");

    for forbidden in [
        "impl UdpDispatch",
        "use crate::runtime::udp_dispatch::UdpDispatch",
    ] {
        assert!(
            !content.contains(forbidden),
            "protocol_runtime::udp::flows should define request types, not extend runtime dispatcher; found `{forbidden}`"
        );
    }
}

#[test]
fn protocol_udp_start_logic_is_split_by_protocol_family() {
    let root = manifest_dir().join("src/protocol_runtime/udp");

    assert!(
        !root.join("start.rs").exists(),
        "protocol UDP start logic should live under src/protocol_runtime/udp/start/, not in a monolithic start.rs"
    );

    for path in [
        "start/mod.rs",
        "start/datagram.rs",
        "start/mieru.rs",
        "start/trojan.rs",
        "start/vless.rs",
        "start/vmess.rs",
    ] {
        assert!(
            root.join(path).exists(),
            "protocol UDP start logic should keep protocol-family module `{path}`"
        );
    }
}

#[test]
fn protocol_udp_datagram_start_keeps_trojan_and_mieru_in_protocol_modules() {
    let datagram = read("src/protocol_runtime/udp/start/datagram.rs");
    let trojan = manifest_dir().join("src/protocol_runtime/udp/start/trojan.rs");
    let mieru = manifest_dir().join("src/protocol_runtime/udp/start/mieru.rs");

    for forbidden in [
        "TrojanUdpFlowRequest",
        "TrojanUdpRelayFlowRequest",
        "MieruUdpFlowRequest",
        "start_mieru_udp_relay_flow",
        "TrojanSendExisting",
        "MieruSendExisting",
    ] {
        assert!(
            !datagram.contains(forbidden),
            "start/datagram.rs should keep Trojan and Mieru start facades in protocol modules; found `{forbidden}`"
        );
    }
    assert!(
        trojan.exists(),
        "Trojan UDP start facade should live in start/trojan.rs"
    );
    assert!(
        mieru.exists(),
        "Mieru UDP start facade should live in start/mieru.rs"
    );
}

#[test]
fn udp_dispatch_keeps_managed_flow_handles_in_udp_flow_module() {
    let dispatch = read("src/runtime/udp_dispatch/mod.rs");
    let lifecycle = read("src/runtime/udp_dispatch/lifecycle.rs");
    let managed = read("src/runtime/udp_flow/managed.rs");

    for source in [&dispatch, &lifecycle] {
        for forbidden in ["HashMap<(Address, u16)", "SessionHandle", "managed_handles"] {
            assert!(
                !source.contains(forbidden),
                "UDP dispatch should keep managed-flow handle storage behind runtime::udp_flow::managed; found `{forbidden}`"
            );
        }
    }
    assert!(
        dispatch.contains("managed_flows: ManagedUdpFlows")
            && lifecycle.contains("ManagedUdpFlows::default()")
            && managed.contains("struct ManagedUdpFlows")
            && managed.contains("SessionHandle"),
        "runtime::udp_flow::managed should own protocol-managed flow handles"
    );
}

#[test]
fn udp_dispatch_does_not_keep_protocol_start_wrappers() {
    let root = manifest_dir().join("src/runtime/udp_dispatch");

    assert!(
        !root.join("protocol_start.rs").exists(),
        "runtime UDP protocol start wrappers should not live beside udp_dispatch root"
    );
    assert!(
        !root.join("start/protocol.rs").exists(),
        "runtime UDP dispatch should not keep broad protocol start wrappers; use narrow per-flow dispatch facades"
    );

    for path in rust_sources_under("src/runtime/udp_dispatch") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        let allowed_facade = match source.as_str() {
            "src/runtime/udp_dispatch/hysteria2_flow.rs" => {
                Some(("Hysteria2DatagramSend", "start_hysteria2_udp_flow"))
            }
            "src/runtime/udp_dispatch/mieru_flow.rs" => {
                Some(("MieruDatagramSend", "start_mieru_udp_flow"))
            }
            "src/runtime/udp_dispatch/shadowsocks_flow.rs" => {
                Some(("ShadowsocksDatagramSend", "start_shadowsocks_udp_flow"))
            }
            "src/runtime/udp_dispatch/trojan_flow.rs" => {
                Some(("TrojanDatagramSend", "start_trojan_udp_flow"))
            }
            "src/runtime/udp_dispatch/vless_flow.rs" => {
                Some(("VlessDatagramSend", "start_vless_udp_flow"))
            }
            "src/runtime/udp_dispatch/vmess_flow.rs" => {
                Some(("VmessDatagramSend", "start_vmess_udp_flow"))
            }
            _ => None,
        };
        if let Some((request, start)) = allowed_facade {
            assert!(
                content.contains(request) && content.contains(start),
                "{source} should own its narrow protocol-state bridge"
            );
            continue;
        }
        for forbidden in [
            "ShadowsocksUdpFlow",
            "MieruUdpRelayFlow",
            "VlessUdpFlow",
            "VlessUdpRelayFinalHop",
            "VlessUdpRelayTwoStream",
            "VmessUdpFlow",
            "VmessUdpRelayFlow",
            "start_shadowsocks_udp_flow",
            "Hysteria2UdpFlowRequest",
            "TrojanUdpFlowRequest",
            "TrojanUdpRelayFlowRequest",
            "MieruUdpFlowRequest",
            "start_mieru_udp_relay_flow",
            "start_vless_udp_flow",
            "start_vless_udp_relay_two_stream",
            "start_vless_udp_relay_final_hop",
            "start_vmess_udp_flow",
            "start_vmess_udp_relay_flow",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should not expose protocol UDP start wrapper `{forbidden}`"
            );
        }
    }
}

#[test]
fn protocol_registry_tests_live_outside_logic_file() {
    let registry = read("src/protocol_adapter/registry.rs");
    let tests = manifest_dir().join("src/protocol_adapter/registry/tests.rs");

    assert!(
        !registry.contains("mod tests {"),
        "protocol registry tests should live in src/protocol_adapter/registry/tests.rs"
    );
    assert!(
        tests.exists(),
        "protocol registry boundary tests should stay in a sibling tests module"
    );
    let tests_content = read("src/protocol_adapter/registry/tests.rs");
    assert!(
        !tests_content.contains("use super::*;"),
        "protocol registry tests should import registry dependencies explicitly"
    );
}

#[test]
fn protocol_registry_tests_root_is_facade_only() {
    let tests = read("src/protocol_adapter/registry/tests.rs");
    let fixtures = read("src/protocol_adapter/registry/tests/fixtures.rs");
    let inbound = read("src/protocol_adapter/registry/tests/inbound.rs");
    let outbound = read("src/protocol_adapter/registry/tests/outbound.rs");

    for expected in ["mod fixtures;", "mod inbound;", "mod outbound;"] {
        assert!(
            tests.contains(expected),
            "src/protocol_adapter/registry/tests.rs should expose test facade item `{expected}`"
        );
    }

    for forbidden in [
        "#[test]",
        "fn compiled_in_inbound_configs",
        "fn compiled_in_outbound_leaves",
        "fn inbound_protocol_name",
        "fn outbound_leaf_name",
        "ResolvedLeafOutbound::",
        "InboundProtocolConfig::",
        "ProtocolRegistry::build()",
    ] {
        assert!(
            !tests.contains(forbidden),
            "src/protocol_adapter/registry/tests.rs should remain a facade over fixtures/inbound/outbound test modules; found `{forbidden}`"
        );
    }

    assert!(
        fixtures.contains("fn compiled_in_inbound_configs")
            && fixtures.contains("fn compiled_in_outbound_leaves")
            && fixtures.contains("fn inbound_protocol_name")
            && fixtures.contains("fn outbound_leaf_name"),
        "src/protocol_adapter/registry/tests/fixtures.rs should own registry test fixtures"
    );
    assert!(
        inbound.contains("compiled_in_inbound_variants_have_exactly_one_registered_adapter"),
        "src/protocol_adapter/registry/tests/inbound.rs should own inbound registry tests"
    );
    assert!(
        outbound.contains("compiled_in_outbound_leaf_variants_have_expected_adapter_claims")
            && outbound.contains("block_outbound_leaf_is_kernel_fact_not_adapter_protocol"),
        "src/protocol_adapter/registry/tests/outbound.rs should own outbound registry tests"
    );
}

#[test]
fn protocol_registry_root_is_facade_only() {
    let registry = read("src/protocol_adapter/registry.rs");

    for expected in [
        "mod build;",
        "mod inbound;",
        "mod metadata;",
        "mod outbound;",
        "mod support;",
        "mod validation;",
        "pub(crate) struct ProtocolRegistry",
        "adapters: Vec<std::sync::Arc<dyn crate::protocol_adapter::RegisteredProtocolCapability>>",
        "impl fmt::Debug for ProtocolRegistry",
    ] {
        assert!(
            registry.contains(expected),
            "src/protocol_adapter/registry.rs should expose registry facade item `{expected}`"
        );
    }

    for forbidden in [
        "pub(crate) fn build",
        "pub(crate) fn register",
        "pub(crate) fn find_inbound",
        "pub(crate) fn find_outbound_leaf",
        "pub(crate) fn bind_inbound",
        "pub(crate) fn inbound_names",
        "pub(crate) fn outbound_names",
        "pub(crate) fn supports_inbound",
        "pub(crate) fn supports_outbound",
        "pub(crate) fn validate_inbounds",
        "pub(crate) fn validate_outbounds",
        "adapter.",
        "InboundProtocolConfig::",
        "OutboundProtocolConfig::",
        "ResolvedLeafOutbound::",
    ] {
        assert!(
            !registry.contains(forbidden),
            "src/protocol_adapter/registry.rs should remain a facade over registry submodules; found `{forbidden}`"
        );
    }
}

#[test]
fn protocol_registry_build_lives_in_register_surface() {
    let adapters = read("src/adapters/mod.rs");
    let registry = read("src/protocol_adapter/registry.rs");
    let build = read("src/protocol_adapter/registry/build.rs");
    let register = read("src/register.rs");
    let inventory = read("src/inventory.rs");

    assert!(
        !adapters.contains("build_registry"),
        "src/adapters/mod.rs should not own registry construction"
    );
    assert!(
        !registry.contains("pub(crate) fn build() -> Self"),
        "src/protocol_adapter/registry.rs should keep registry construction out of the registry facade"
    );
    assert!(
        !build.contains("pub(crate) fn build() -> Self"),
        "src/protocol_adapter/registry/build.rs should only own the low-level register helper"
    );
    assert!(
        register.contains("pub(crate) fn protocol_registry() -> ProtocolRegistry"),
        "src/register.rs should own compiled protocol registry construction"
    );
    assert!(
        inventory.contains("crate::register::protocol_registry()"),
        "src/inventory.rs should build the registry through the register surface"
    );
}

#[test]
fn protocol_adapter_imports_live_in_register_surface() {
    let registry = read("src/protocol_adapter/registry.rs");
    let build = read("src/protocol_adapter/registry/build.rs");
    let register = read("src/register.rs");

    for adapter in [
        "DirectAdapter",
        "HttpConnectAdapter",
        "Hysteria2Adapter",
        "MieruAdapter",
        "MixedAdapter",
        "ShadowsocksAdapter",
        "Socks5Adapter",
        "TrojanAdapter",
        "VlessAdapter",
        "VmessAdapter",
    ] {
        assert!(
            !registry.contains(adapter) && !build.contains(adapter),
            "protocol_adapter registry modules should keep concrete adapter imports in src/register.rs; found `{adapter}`"
        );
        assert!(
            register.contains(adapter),
            "src/register.rs should own concrete adapter import `{adapter}`"
        );
    }
}

#[test]
fn protocol_registry_register_helper_stays_in_build_module() {
    let registry = read("src/protocol_adapter/registry.rs");
    let build = read("src/protocol_adapter/registry/build.rs");

    assert!(
        !registry.contains("pub(crate) fn register("),
        "src/protocol_adapter/registry.rs should keep register helper in src/protocol_adapter/registry/build.rs"
    );
    assert!(
        build.contains("pub(crate) fn register<T>(&mut self, adapter: std::sync::Arc<T>)"),
        "src/protocol_adapter/registry/build.rs should own the register helper used by src/register.rs"
    );
    assert!(
        build.contains("T: ProtocolAdapter + RegisteredProtocolCapability + 'static")
            && build.contains("std::sync::Arc<dyn RegisteredProtocolCapability>"),
        "src/protocol_adapter/registry/build.rs should adapt registered ProtocolAdapter values into capability objects"
    );
}

#[test]
fn protocol_registry_metadata_lives_in_metadata_module() {
    let registry = read("src/protocol_adapter/registry.rs");
    let metadata = read("src/protocol_adapter/registry/metadata.rs");

    for forbidden in [
        "pub(crate) fn inbound_names",
        "pub(crate) fn outbound_names",
        "pub(crate) fn capabilities",
    ] {
        assert!(
            !registry.contains(forbidden),
            "src/protocol_adapter/registry.rs should keep metadata methods in src/protocol_adapter/registry/metadata.rs; found `{forbidden}`"
        );
        assert!(
            metadata.contains(forbidden),
            "src/protocol_adapter/registry/metadata.rs should own registry metadata method `{forbidden}`"
        );
    }
}

#[test]
fn protocol_registry_support_lives_in_support_module() {
    let registry = read("src/protocol_adapter/registry.rs");
    let metadata = read("src/protocol_adapter/registry/metadata.rs");
    let support = read("src/protocol_adapter/registry/support.rs");

    for forbidden in [
        "pub(crate) fn supports_inbound",
        "pub(crate) fn supports_outbound",
        "pub(crate) fn inbound_protocol_label",
        "pub(crate) fn inbound_protocol_feature_name",
        "pub(crate) fn outbound_protocol_label",
        "pub(crate) fn outbound_protocol_feature_name",
    ] {
        assert!(
            !registry.contains(forbidden),
            "src/protocol_adapter/registry.rs should keep support methods in src/protocol_adapter/registry/support.rs; found `{forbidden}`"
        );
        assert!(
            !metadata.contains(forbidden),
            "src/protocol_adapter/registry/metadata.rs should keep support methods in src/protocol_adapter/registry/support.rs; found `{forbidden}`"
        );
        assert!(
            support.contains(forbidden),
            "src/protocol_adapter/registry/support.rs should own registry support method `{forbidden}`"
        );
    }
}

#[test]
fn protocol_registry_validation_lives_in_validation_module() {
    let registry = read("src/protocol_adapter/registry.rs");
    let metadata = read("src/protocol_adapter/registry/metadata.rs");
    let validation = read("src/protocol_adapter/registry/validation.rs");

    for forbidden in [
        "pub(crate) fn validate_inbounds",
        "pub(crate) fn validate_outbounds",
    ] {
        assert!(
            !registry.contains(forbidden),
            "src/protocol_adapter/registry.rs should keep validation methods in src/protocol_adapter/registry/validation.rs; found `{forbidden}`"
        );
        assert!(
            !metadata.contains(forbidden),
            "src/protocol_adapter/registry/metadata.rs should keep validation methods in src/protocol_adapter/registry/validation.rs; found `{forbidden}`"
        );
        assert!(
            validation.contains(forbidden),
            "src/protocol_adapter/registry/validation.rs should own registry validation method `{forbidden}`"
        );
    }
}

#[test]
fn protocol_registry_outbound_dispatch_lives_in_outbound_module() {
    let registry = read("src/protocol_adapter/registry.rs");
    let outbound = read("src/protocol_adapter/registry/outbound.rs");

    for forbidden in [
        "pub(crate) fn find_outbound_leaf",
        "pub(crate) fn outbound_leaf_runtime",
        "ResolvedLeafOutbound::Block",
        "TcpPathCategory::Block",
    ] {
        assert!(
            !registry.contains(forbidden),
            "src/protocol_adapter/registry.rs should keep outbound dispatch in src/protocol_adapter/registry/outbound.rs; found `{forbidden}`"
        );
        assert!(
            outbound.contains(forbidden),
            "src/protocol_adapter/registry/outbound.rs should own outbound dispatch item `{forbidden}`"
        );
    }
}

#[test]
fn protocol_registry_inbound_dispatch_lives_in_inbound_module() {
    let registry = read("src/protocol_adapter/registry.rs");
    let inbound = read("src/protocol_adapter/registry/inbound.rs");

    for forbidden in [
        "pub(crate) fn find_inbound",
        "pub(crate) async fn bind_inbound",
        "InboundListenerCapability::bind_inbound(",
    ] {
        assert!(
            !registry.contains(forbidden),
            "src/protocol_adapter/registry.rs should keep inbound dispatch in src/protocol_adapter/registry/inbound.rs; found `{forbidden}`"
        );
        assert!(
            inbound.contains(forbidden),
            "src/protocol_adapter/registry/inbound.rs should own inbound dispatch item `{forbidden}`"
        );
    }
}

#[test]
fn protocol_adapter_dispatch_is_not_public_api() {
    let root = read("src/protocol_adapter.rs");
    let registry = read("src/protocol_adapter/registry.rs");
    let adapter = read("src/protocol_adapter/adapter.rs");
    let capability = read("src/protocol_adapter/capability.rs");

    for forbidden in [
        "pub use registry::ProtocolRegistry;",
        "pub trait ProtocolAdapter",
        "pub struct ProtocolRegistry",
    ] {
        assert!(
            !root.contains(forbidden) && !registry.contains(forbidden),
            "protocol adapter dispatch internals should stay crate-private; found `{forbidden}`"
        );
    }

    assert!(
        root.contains("pub(crate) use registry::ProtocolRegistry;"),
        "src/protocol_adapter.rs should keep ProtocolRegistry visible only inside zero-proxy"
    );
    assert!(
        root.contains("pub(crate) use adapter::ProtocolAdapter;"),
        "src/protocol_adapter.rs should re-export ProtocolAdapter crate-privately"
    );
    assert!(
        adapter.contains("pub(crate) trait ProtocolAdapter"),
        "src/protocol_adapter/adapter.rs should own the ProtocolAdapter trait definition"
    );
    assert!(
        capability.contains("pub(crate) trait ProtocolSupportCapability"),
        "src/protocol_adapter/capability.rs should own focused adapter capability traits"
    );
    assert!(
        registry.contains("pub(crate) struct ProtocolRegistry"),
        "src/protocol_adapter/registry.rs should keep ProtocolRegistry visible only inside zero-proxy"
    );
}

#[test]
fn protocol_adapter_root_is_facade_only() {
    let root = read("src/protocol_adapter.rs");

    for expected in [
        "mod adapter;",
        "mod capability;",
        "mod context;",
        "mod defaults;",
        "mod model;",
        "mod registry;",
        "pub(crate) use adapter::ProtocolAdapter;",
        "pub(crate) use capability::",
        "pub(crate) use context::{InboundAdapterContext, OutboundAdapterContext, UdpAdapterContext};",
        "pub(crate) use model::{BoundInbound, OutboundLeafRuntime};",
        "pub(crate) use registry::ProtocolRegistry;",
    ] {
        assert!(
            root.contains(expected),
            "src/protocol_adapter.rs should expose facade item `{expected}`"
        );
    }

    for forbidden in [
        "trait ProtocolAdapter",
        "struct ProtocolRegistry",
        "enum BoundInbound",
        "struct OutboundLeafRuntime",
        "impl ProtocolAdapter",
        "impl ProtocolRegistry",
        "async fn",
        "fn find_outbound_leaf",
        "fn find_inbound",
        "InboundProtocolConfig::",
        "OutboundProtocolConfig::",
        "ResolvedLeafOutbound::",
    ] {
        assert!(
            !root.contains(forbidden),
            "src/protocol_adapter.rs should remain a facade over adapter/defaults/model/registry modules; found `{forbidden}`"
        );
    }
}

#[test]
fn protocol_adapter_capabilities_are_split_by_responsibility() {
    let root = read("src/protocol_adapter.rs");
    let adapter = read("src/protocol_adapter/adapter.rs");
    let capability = read("src/protocol_adapter/capability.rs");
    let context = read("src/protocol_adapter/context.rs");

    for expected in [
        "pub(crate) trait RegisteredProtocolCapability",
        "pub(crate) trait ProtocolSupportCapability",
        "pub(crate) trait InboundListenerCapability",
        "pub(crate) trait TcpOutboundCapability",
        "pub(crate) trait UdpFlowCapability",
        "pub(crate) trait UdpPacketPathCapability",
    ] {
        assert!(
            capability.contains(expected),
            "src/protocol_adapter/capability.rs should expose focused capability trait `{expected}`"
        );
    }

    assert!(
        root.contains("mod capability;"),
        "src/protocol_adapter.rs should wire the capability trait module"
    );
    assert!(
        root.contains("mod context;"),
        "src/protocol_adapter.rs should wire the adapter context module"
    );
    for expected in [
        "pub(crate) struct InboundAdapterContext",
        "pub(crate) struct OutboundAdapterContext",
        "pub(crate) struct UdpAdapterContext",
    ] {
        assert!(
            context.contains(expected),
            "src/protocol_adapter/context.rs should expose narrow adapter context `{expected}`"
        );
    }
    assert!(
        adapter.contains("pub(crate) trait ProtocolAdapter"),
        "src/protocol_adapter/adapter.rs should keep the compatibility adapter trait"
    );
    assert!(
        capability.contains("impl<T> RegisteredProtocolCapability for T"),
        "src/protocol_adapter/capability.rs should provide the registry collector blanket impl"
    );
    assert!(
        !capability.contains("impl<T> TcpOutboundCapability for T"),
        "TCP outbound dispatch should use explicit TcpOutboundCapability impls, not a ProtocolAdapter blanket shim"
    );
    assert!(
        !capability.contains("impl<T> InboundListenerCapability for T"),
        "inbound listener dispatch should use explicit InboundListenerCapability impls, not a ProtocolAdapter blanket shim"
    );
    assert!(
        !capability.contains("impl<T> UdpFlowCapability for T"),
        "UDP flow dispatch should use explicit UdpFlowCapability impls, not a ProtocolAdapter blanket shim"
    );
    assert!(
        !capability.contains("impl<T> UdpPacketPathCapability for T"),
        "UDP packet-path dispatch should use explicit UdpPacketPathCapability impls, not a ProtocolAdapter blanket shim"
    );
}

#[test]
fn protocol_support_capability_is_not_on_monolithic_adapter() {
    let adapter = read("src/protocol_adapter/adapter.rs");
    let capability = read("src/protocol_adapter/capability.rs");

    for forbidden in [
        "fn name(&self)",
        "fn feature_name(&self)",
        "fn supports_inbound(&self",
        "fn supports_outbound(&self",
        "fn has_inbound(&self)",
        "fn has_outbound(&self)",
        "impl<T> ProtocolSupportCapability for T",
    ] {
        assert!(
            !adapter.contains(forbidden)
                && (forbidden != "fn name(&self)" || !capability.contains("ProtocolAdapter::name"))
                && (forbidden != "fn feature_name(&self)"
                    || !capability.contains("ProtocolAdapter::feature_name"))
                && (forbidden != "fn supports_inbound(&self"
                    || !capability.contains("ProtocolAdapter::supports_inbound"))
                && (forbidden != "fn supports_outbound(&self"
                    || !capability.contains("ProtocolAdapter::supports_outbound"))
                && (forbidden != "fn has_inbound(&self)"
                    || !capability.contains("ProtocolAdapter::has_inbound"))
                && (forbidden != "fn has_outbound(&self)"
                    || !capability.contains("ProtocolAdapter::has_outbound"))
                && (forbidden != "impl<T> ProtocolSupportCapability for T"
                    || !capability.contains(forbidden)),
            "protocol metadata/support should live in explicit ProtocolSupportCapability impls, not `{forbidden}`"
        );
    }

    for source in [
        "src/adapters/direct.rs",
        "src/adapters/http_connect.rs",
        "src/adapters/hysteria2.rs",
        "src/adapters/mieru.rs",
        "src/adapters/mixed.rs",
        "src/adapters/shadowsocks.rs",
        "src/adapters/socks5.rs",
        "src/adapters/trojan.rs",
        "src/adapters/vless.rs",
        "src/adapters/vmess.rs",
    ] {
        let content = read(source);
        assert!(
            content.contains("impl ProtocolSupportCapability for"),
            "{source} should explicitly implement ProtocolSupportCapability"
        );
    }
}

#[test]
fn tcp_outbound_capability_is_not_on_monolithic_adapter() {
    let adapter = read("src/protocol_adapter/adapter.rs");
    let capability = read("src/protocol_adapter/capability.rs");

    for forbidden in [
        "fn claims_outbound_leaf(&self",
        "fn outbound_leaf_runtime",
        "async fn connect_tcp",
        "async fn apply_relay_hop",
    ] {
        assert!(
            !adapter.contains(forbidden),
            "TCP outbound capability should not remain on ProtocolAdapter surface `{forbidden}`"
        );
    }

    for forbidden in [
        "ProtocolAdapter::claims_outbound_leaf",
        "ProtocolAdapter::outbound_leaf_runtime",
        "ProtocolAdapter::connect_tcp",
        "ProtocolAdapter::apply_relay_hop",
    ] {
        assert!(
            !capability.contains(forbidden),
            "TCP outbound capability should be implemented explicitly, not through ProtocolAdapter surface `{forbidden}`"
        );
    }
}

#[test]
fn inbound_listener_capability_is_not_on_monolithic_adapter() {
    let adapter = read("src/protocol_adapter/adapter.rs");
    let capability = read("src/protocol_adapter/capability.rs");

    for forbidden in ["async fn bind_inbound", "fn spawn_inbound"] {
        assert!(
            !adapter.contains(forbidden),
            "inbound listener capability should not remain on ProtocolAdapter surface `{forbidden}`"
        );
    }

    for forbidden in [
        "ProtocolAdapter::bind_inbound",
        "ProtocolAdapter::spawn_inbound",
    ] {
        assert!(
            !capability.contains(forbidden),
            "inbound listener capability should be implemented explicitly, not through ProtocolAdapter surface `{forbidden}`"
        );
    }
}

#[test]
fn udp_flow_capability_is_not_on_monolithic_adapter() {
    let adapter = read("src/protocol_adapter/adapter.rs");
    let capability = read("src/protocol_adapter/capability.rs");

    for forbidden in [
        "async fn start_udp_flow",
        "fn udp_relay_needs_two_streams",
        "async fn start_udp_relay_two_stream",
        "async fn start_udp_relay_final_hop",
    ] {
        assert!(
            !adapter.contains(forbidden),
            "UDP flow capability should not remain on ProtocolAdapter surface `{forbidden}`"
        );
    }

    for forbidden in [
        "ProtocolAdapter::start_udp_flow",
        "ProtocolAdapter::udp_relay_needs_two_streams",
        "ProtocolAdapter::start_udp_relay_two_stream",
        "ProtocolAdapter::start_udp_relay_final_hop",
    ] {
        assert!(
            !capability.contains(forbidden),
            "UDP flow capability should be implemented explicitly, not through ProtocolAdapter surface `{forbidden}`"
        );
    }
}

#[test]
fn udp_packet_path_capability_is_not_on_monolithic_adapter() {
    let adapter = read("src/protocol_adapter/adapter.rs");
    let capability = read("src/protocol_adapter/capability.rs");

    for forbidden in [
        "fn udp_packet_path_carrier_descriptor",
        "async fn build_udp_packet_path",
        "fn udp_datagram_source",
    ] {
        assert!(
            !adapter.contains(forbidden),
            "UDP packet-path capability should not remain on ProtocolAdapter surface `{forbidden}`"
        );
    }

    for forbidden in [
        "ProtocolAdapter::udp_packet_path_carrier_descriptor",
        "ProtocolAdapter::build_udp_packet_path",
        "ProtocolAdapter::udp_datagram_source",
    ] {
        assert!(
            !capability.contains(forbidden),
            "UDP packet-path capability should be implemented explicitly, not through ProtocolAdapter surface `{forbidden}`"
        );
    }
}

#[test]
fn registered_adapters_implement_inbound_listener_capability_explicitly() {
    for (source, adapter) in [
        ("src/adapters/direct.rs", "DirectAdapter"),
        ("src/adapters/http_connect.rs", "HttpConnectAdapter"),
        ("src/adapters/hysteria2.rs", "Hysteria2Adapter"),
        ("src/adapters/mieru.rs", "MieruAdapter"),
        ("src/adapters/mixed.rs", "MixedAdapter"),
        ("src/adapters/shadowsocks.rs", "ShadowsocksAdapter"),
        ("src/adapters/socks5.rs", "Socks5Adapter"),
        ("src/adapters/trojan.rs", "TrojanAdapter"),
        ("src/adapters/vless.rs", "VlessAdapter"),
        ("src/adapters/vmess.rs", "VmessAdapter"),
    ] {
        let content = read(source);
        assert!(
            content.contains(&format!("impl InboundListenerCapability for {adapter}")),
            "{source} should explicitly implement InboundListenerCapability for {adapter}"
        );
    }
}

#[test]
fn registered_adapters_implement_udp_flow_capability_explicitly() {
    for (source, adapter) in [
        ("src/adapters/direct.rs", "DirectAdapter"),
        ("src/adapters/http_connect.rs", "HttpConnectAdapter"),
        ("src/adapters/hysteria2.rs", "Hysteria2Adapter"),
        ("src/adapters/mieru.rs", "MieruAdapter"),
        ("src/adapters/mixed.rs", "MixedAdapter"),
        ("src/adapters/shadowsocks.rs", "ShadowsocksAdapter"),
        ("src/adapters/socks5.rs", "Socks5Adapter"),
        ("src/adapters/trojan.rs", "TrojanAdapter"),
        ("src/adapters/vless.rs", "VlessAdapter"),
        ("src/adapters/vmess.rs", "VmessAdapter"),
    ] {
        let content = read(source);
        assert!(
            content.contains(&format!("impl UdpFlowCapability for {adapter}")),
            "{source} should explicitly implement UdpFlowCapability for {adapter}"
        );
    }
}

#[test]
fn registered_adapters_implement_udp_packet_path_capability_explicitly() {
    for (source, adapter) in [
        ("src/adapters/direct.rs", "DirectAdapter"),
        ("src/adapters/http_connect.rs", "HttpConnectAdapter"),
        ("src/adapters/hysteria2.rs", "Hysteria2Adapter"),
        ("src/adapters/mieru.rs", "MieruAdapter"),
        ("src/adapters/mixed.rs", "MixedAdapter"),
        ("src/adapters/shadowsocks.rs", "ShadowsocksAdapter"),
        ("src/adapters/socks5.rs", "Socks5Adapter"),
        ("src/adapters/trojan.rs", "TrojanAdapter"),
        ("src/adapters/vless.rs", "VlessAdapter"),
        ("src/adapters/vmess.rs", "VmessAdapter"),
    ] {
        let content = read(source);
        assert!(
            content.contains(&format!("impl UdpPacketPathCapability for {adapter}")),
            "{source} should explicitly implement UdpPacketPathCapability for {adapter}"
        );
    }
}

#[test]
fn registered_adapters_implement_tcp_outbound_capability_explicitly() {
    for (source, adapter) in [
        ("src/adapters/direct.rs", "DirectAdapter"),
        ("src/adapters/http_connect.rs", "HttpConnectAdapter"),
        ("src/adapters/hysteria2.rs", "Hysteria2Adapter"),
        ("src/adapters/mieru.rs", "MieruAdapter"),
        ("src/adapters/mixed.rs", "MixedAdapter"),
        ("src/adapters/shadowsocks.rs", "ShadowsocksAdapter"),
        ("src/adapters/socks5.rs", "Socks5Adapter"),
        ("src/adapters/trojan.rs", "TrojanAdapter"),
        ("src/adapters/vless.rs", "VlessAdapter"),
        ("src/adapters/vmess.rs", "VmessAdapter"),
    ] {
        let content = read(source);
        assert!(
            content.contains(&format!("impl TcpOutboundCapability for {adapter}")),
            "{source} should explicitly implement TcpOutboundCapability for {adapter}"
        );
    }
}

#[test]
fn protocol_registry_stores_capability_objects() {
    let registry = read("src/protocol_adapter/registry.rs");
    let inbound = read("src/protocol_adapter/registry/inbound.rs");
    let outbound = read("src/protocol_adapter/registry/outbound.rs");

    assert!(
        registry.contains("RegisteredProtocolCapability"),
        "ProtocolRegistry should store registered capability objects"
    );
    for forbidden in [
        "Vec<std::sync::Arc<dyn crate::protocol_adapter::ProtocolAdapter>>",
        "Result<Arc<dyn ProtocolAdapter>",
    ] {
        assert!(
            !registry.contains(forbidden)
                && !inbound.contains(forbidden)
                && !outbound.contains(forbidden),
            "ProtocolRegistry dispatch should not expose monolithic adapter object `{forbidden}`"
        );
    }
}

#[test]
fn protocol_adapter_capabilities_use_contexts_not_proxy() {
    let adapter = read("src/protocol_adapter/adapter.rs");
    let capability = read("src/protocol_adapter/capability.rs");

    for forbidden in ["proxy: &Proxy", "_proxy: &Proxy"] {
        assert!(
            !adapter.contains(forbidden) && !capability.contains(forbidden),
            "adapter dispatch traits should receive narrow adapter contexts, not expose `{forbidden}`"
        );
    }

    assert!(
        !adapter.contains("UdpAdapterContext<'_>") && capability.contains("UdpAdapterContext<'_>"),
        "UDP adapter context should live on UDP capability traits, not ProtocolAdapter"
    );
    assert!(
        !adapter.contains("InboundAdapterContext<'_>")
            && capability.contains("InboundAdapterContext<'_>"),
        "inbound listener context should live on InboundListenerCapability, not ProtocolAdapter"
    );
    assert!(
        !adapter.contains("OutboundAdapterContext<'_>")
            && capability.contains("OutboundAdapterContext<'_>"),
        "TCP outbound context should live on TcpOutboundCapability, not ProtocolAdapter"
    );
}

#[test]
fn protocol_adapter_models_live_outside_trait_root() {
    let root = read("src/protocol_adapter.rs");
    let model = read("src/protocol_adapter/model.rs");
    let inbound = read("src/protocol_adapter/model/inbound.rs");
    let outbound = read("src/protocol_adapter/model/outbound.rs");

    for forbidden in ["pub(crate) enum BoundInbound", "impl BoundInbound"] {
        assert!(
            !root.contains(forbidden) && !model.contains(forbidden),
            "src/protocol_adapter.rs and src/protocol_adapter/model.rs should keep inbound adapter models in src/protocol_adapter/model/inbound.rs; found `{forbidden}`"
        );
        assert!(
            inbound.contains(forbidden),
            "src/protocol_adapter/model/inbound.rs should own adapter inbound model `{forbidden}`"
        );
    }

    for forbidden in [
        "pub(crate) struct OutboundLeafRuntime",
        "use crate::runtime::orchestration::{OutboundEndpoint, TcpPathCategory}",
    ] {
        assert!(
            !root.contains(forbidden) && !model.contains(forbidden),
            "src/protocol_adapter.rs and src/protocol_adapter/model.rs should keep outbound adapter models in src/protocol_adapter/model/outbound.rs; found `{forbidden}`"
        );
        assert!(
            outbound.contains(forbidden),
            "src/protocol_adapter/model/outbound.rs should own adapter outbound model `{forbidden}`"
        );
    }

    for forbidden in [
        "pub(crate) enum BoundInbound",
        "pub(crate) struct OutboundLeafRuntime",
        "impl BoundInbound",
        "use crate::runtime::orchestration::{OutboundEndpoint, TcpPathCategory}",
    ] {
        assert!(
            !root.contains(forbidden),
            "src/protocol_adapter.rs should keep adapter models in src/protocol_adapter/model.rs; found `{forbidden}`"
        );
    }
    assert!(
        root.contains("pub(crate) use model::{BoundInbound, OutboundLeafRuntime};"),
        "src/protocol_adapter.rs should re-export adapter models crate-privately"
    );
}

#[test]
fn protocol_adapter_model_root_is_facade_only() {
    let model = read("src/protocol_adapter/model.rs");

    for expected in [
        "mod inbound;",
        "mod outbound;",
        "pub(crate) use inbound::BoundInbound;",
        "pub(crate) use outbound::OutboundLeafRuntime;",
    ] {
        assert!(
            model.contains(expected),
            "src/protocol_adapter/model.rs should expose model facade item `{expected}`"
        );
    }

    for forbidden in [
        "enum BoundInbound",
        "struct OutboundLeafRuntime",
        "impl BoundInbound",
        "TcpPathCategory",
        "OutboundEndpoint",
        "TokioListener",
        "QuicInbound",
        "into_tcp",
    ] {
        assert!(
            !model.contains(forbidden),
            "src/protocol_adapter/model.rs should remain a facade over inbound/outbound model modules; found `{forbidden}`"
        );
    }
}

#[test]
fn protocol_adapter_default_errors_live_outside_trait_root() {
    let root = read("src/protocol_adapter.rs");
    let defaults = read("src/protocol_adapter/defaults.rs");
    let errors = read("src/protocol_adapter/defaults/errors.rs");

    for forbidden in [
        "std::io::ErrorKind::Unsupported",
        "TcpOutboundFailure {",
        "FlowFailure {",
        "no_tcp_outbound",
        "no_udp_outbound",
        "no_two_stream_relay",
        "no_udp_relay_final_hop",
    ] {
        assert!(
            !root.contains(forbidden),
            "src/protocol_adapter.rs should keep default unsupported error construction in src/protocol_adapter/defaults/errors.rs; found `{forbidden}`"
        );
        assert!(
            !defaults.contains(forbidden),
            "src/protocol_adapter/defaults.rs should keep default unsupported error construction in src/protocol_adapter/defaults/errors.rs; found `{forbidden}`"
        );
        assert!(
            errors.contains(forbidden),
            "src/protocol_adapter/defaults/errors.rs should own default unsupported error construction `{forbidden}`"
        );
    }
}

#[test]
fn protocol_adapter_default_tcp_bind_lives_outside_trait_root() {
    let root = read("src/protocol_adapter/adapter.rs");
    let defaults = read("src/protocol_adapter/defaults.rs");
    let bind = read("src/protocol_adapter/defaults/bind.rs");

    for forbidden in ["TokioListener::bind", "BoundInbound::Tcp"] {
        assert!(
            !root.contains(forbidden),
            "src/protocol_adapter/adapter.rs should keep default TCP bind construction in src/protocol_adapter/defaults/bind.rs; found `{forbidden}`"
        );
        assert!(
            !defaults.contains(forbidden),
            "src/protocol_adapter/defaults.rs should keep default TCP bind construction in src/protocol_adapter/defaults/bind.rs; found `{forbidden}`"
        );
        assert!(
            bind.contains(forbidden),
            "src/protocol_adapter/defaults/bind.rs should own default TCP bind construction `{forbidden}`"
        );
    }
}

#[test]
fn protocol_adapter_defaults_root_is_facade_only() {
    let defaults = read("src/protocol_adapter/defaults.rs");

    for expected in [
        "mod bind;",
        "mod errors;",
        "pub(super) use bind::bind_tcp_inbound;",
        "pub(super) use errors::{",
    ] {
        assert!(
            defaults.contains(expected),
            "src/protocol_adapter/defaults.rs should expose defaults facade item `{expected}`"
        );
    }

    for forbidden in [
        "async fn",
        "fn unsupported_io",
        "fn udp_flow_unsupported",
        "TcpOutboundFailure {",
        "FlowFailure {",
        "TokioListener::bind",
        "BoundInbound::Tcp",
        "std::io::ErrorKind::Unsupported",
    ] {
        assert!(
            !defaults.contains(forbidden),
            "src/protocol_adapter/defaults.rs should remain a facade over bind/errors modules; found `{forbidden}`"
        );
    }
}

#[test]
fn inventory_does_not_expose_adapter_trait_objects() {
    let inventory = read("src/inventory.rs");

    for forbidden in [
        "Arc<dyn crate::protocol_adapter::ProtocolAdapter>",
        "Arc<dyn ProtocolAdapter>",
        "pub(crate) fn find_outbound_leaf",
        "pub(crate) fn find_inbound",
    ] {
        assert!(
            !inventory.contains(forbidden),
            "src/inventory.rs should expose protocol operations, not adapter trait objects; found `{forbidden}`"
        );
    }
}

#[test]
fn inventory_root_is_facade_only() {
    let root = read("src/inventory.rs");

    for expected in [
        "mod inbound;",
        "mod metadata;",
        "mod runtime;",
        "mod tcp;",
        "mod udp;",
        "pub struct ProtocolInventory",
        "registry: ProtocolRegistry",
    ] {
        assert!(
            root.contains(expected),
            "src/inventory.rs should expose facade item `{expected}`"
        );
    }

    for forbidden in [
        "find_inbound",
        "find_outbound_leaf",
        "adapter.",
        "async fn",
        "InboundProtocolConfig::",
        "OutboundProtocolConfig::",
        "ResolvedLeafOutbound::",
        "FlowFailure",
        "EngineError",
    ] {
        assert!(
            !root.contains(forbidden),
            "src/inventory.rs should remain a facade over inventory submodules; found `{forbidden}`"
        );
    }
}

#[test]
fn inventory_metadata_facade_lives_in_metadata_module() {
    let root = read("src/inventory.rs");
    let metadata = read("src/inventory/metadata.rs");

    for forbidden in [
        "supported_inbounds",
        "supported_outbounds",
        "protocol_capabilities",
        "validate_config",
        "supports_inbound_protocol",
        "supports_outbound_protocol",
    ] {
        assert!(
            !root.contains(forbidden),
            "src/inventory.rs should keep metadata facade methods in src/inventory/metadata.rs; found `{forbidden}`"
        );
        assert!(
            metadata.contains(forbidden),
            "src/inventory/metadata.rs should own metadata facade method `{forbidden}`"
        );
    }
}

#[test]
fn inventory_runtime_facts_live_in_runtime_module() {
    let root = read("src/inventory.rs");
    let runtime = read("src/inventory/runtime.rs");

    for forbidden in ["OutboundLeafRuntime", "outbound_leaf_runtime"] {
        assert!(
            !root.contains(forbidden),
            "src/inventory.rs should keep runtime fact lookup in src/inventory/runtime.rs; found `{forbidden}`"
        );
        assert!(
            runtime.contains(forbidden),
            "src/inventory/runtime.rs should own runtime fact lookup `{forbidden}`"
        );
    }
}

#[test]
fn inventory_udp_adapter_dispatch_lives_in_udp_module() {
    let root = read("src/inventory.rs");
    let udp = read("src/inventory/udp.rs");
    let leaf = read("src/inventory/udp/leaf.rs");
    let relay = read("src/inventory/udp/relay.rs");
    let packet_path = read("src/inventory/udp/packet_path.rs");

    {
        let forbidden = "start_udp_leaf_flow";
        assert!(
            !root.contains(forbidden) && !udp.contains(forbidden),
            "src/inventory.rs and src/inventory/udp.rs should keep UDP leaf dispatch in src/inventory/udp/leaf.rs; found `{forbidden}`"
        );
        assert!(
            leaf.contains(forbidden),
            "src/inventory/udp/leaf.rs should own UDP leaf dispatch method `{forbidden}`"
        );
    }

    for forbidden in [
        "udp_relay_needs_two_streams",
        "start_udp_relay_two_stream",
        "start_udp_relay_final_hop",
    ] {
        assert!(
            !root.contains(forbidden) && !udp.contains(forbidden),
            "src/inventory.rs and src/inventory/udp.rs should keep UDP relay dispatch in src/inventory/udp/relay.rs; found `{forbidden}`"
        );
        assert!(
            relay.contains(forbidden),
            "src/inventory/udp/relay.rs should own UDP relay dispatch method `{forbidden}`"
        );
    }

    for forbidden in [
        "udp_packet_path_pair",
        "resolve_udp_packet_path_candidate",
        "build_udp_packet_path_carrier",
    ] {
        assert!(
            !root.contains(forbidden) && !udp.contains(forbidden),
            "src/inventory.rs and src/inventory/udp.rs should keep UDP packet-path dispatch in src/inventory/udp/packet_path.rs; found `{forbidden}`"
        );
        assert!(
            packet_path.contains(forbidden),
            "src/inventory/udp/packet_path.rs should own UDP packet-path dispatch method `{forbidden}`"
        );
    }
}

#[test]
fn inventory_udp_root_is_facade_only() {
    let udp = read("src/inventory/udp.rs");

    assert!(
        udp.contains("mod leaf;") && udp.contains("mod relay;") && udp.contains("mod packet_path;"),
        "src/inventory/udp.rs should expose the UDP inventory submodules"
    );

    for forbidden in [
        "impl ProtocolInventory",
        "find_outbound_leaf",
        "adapter.",
        "FlowFailure",
        "EngineError",
        "ResolvedLeafOutbound",
    ] {
        assert!(
            !udp.contains(forbidden),
            "src/inventory/udp.rs should remain a facade over leaf/relay/packet_path modules; found `{forbidden}`"
        );
    }
}

#[test]
fn inventory_tcp_adapter_dispatch_lives_in_tcp_module() {
    let root = read("src/inventory.rs");
    let tcp = read("src/inventory/tcp.rs");

    for forbidden in ["connect_tcp_leaf", "apply_tcp_relay_hop"] {
        assert!(
            !root.contains(forbidden),
            "src/inventory.rs should keep TCP adapter dispatch in src/inventory/tcp.rs; found `{forbidden}`"
        );
        assert!(
            tcp.contains(forbidden),
            "src/inventory/tcp.rs should own TCP adapter dispatch method `{forbidden}`"
        );
    }
}

#[test]
fn inventory_inbound_adapter_dispatch_lives_in_inbound_module() {
    let root = read("src/inventory.rs");
    let inbound = read("src/inventory/inbound.rs");

    for forbidden in ["check_inbound_enabled", "bind_inbound", "spawn_inbound"] {
        assert!(
            !root.contains(forbidden),
            "src/inventory.rs should keep inbound adapter dispatch in src/inventory/inbound.rs; found `{forbidden}`"
        );
        assert!(
            inbound.contains(forbidden),
            "src/inventory/inbound.rs should own inbound adapter dispatch method `{forbidden}`"
        );
    }
}

#[test]
fn transport_facade_exports_are_explicit() {
    let content = read("src/transport/mod.rs");

    for forbidden in [
        "pub(crate) use direct::*;",
        "pub(crate) use metered::*;",
        "pub(crate) use stream::*;",
        "pub(crate) use tcp_flow::*;",
        "pub(crate) use tcp_outbound::*;",
        "pub(crate) use tcp_relay::*;",
    ] {
        assert!(
            !content.contains(forbidden),
            "transport facade should explicitly list exported items; found `{forbidden}`"
        );
    }
}

#[test]
fn udp_dispatch_does_not_reexport_protocol_runtime_udp_types() {
    let content = read("src/runtime/udp_dispatch/mod.rs");

    for forbidden in [
        "pub(crate) use crate::protocol_runtime::udp::",
        "pub(crate) use crate::protocol_runtime::socks5_udp::",
    ] {
        assert!(
            !content.contains(forbidden),
            "src/runtime/udp_dispatch/mod.rs should not re-export protocol-runtime UDP types; found `{forbidden}`"
        );
    }
}

#[test]
fn udp_dispatch_root_does_not_reexport_protocol_flow_requests() {
    let content = read("src/runtime/udp_dispatch/mod.rs");

    for forbidden in [
        "pub(crate) use hysteria2_flow::",
        "pub(crate) use mieru_flow::",
        "pub(crate) use shadowsocks_flow::",
        "pub(crate) use socks5_flow::",
        "pub(crate) use trojan_flow::",
        "pub(crate) use vless_flow::",
        "pub(crate) use vmess_flow::",
        "Hysteria2DatagramSend",
        "MieruDatagramSend",
        "MieruRelaySend",
        "ShadowsocksDatagramSend",
        "Socks5RelaySend",
        "TrojanDatagramSend",
        "TrojanRelaySend",
        "VlessDatagramSend",
        "VlessRelayFinalHopSend",
        "VlessRelayTwoStreamSend",
        "VmessDatagramSend",
        "VmessRelaySend",
    ] {
        assert!(
            !content.contains(forbidden),
            "src/runtime/udp_dispatch/mod.rs should not re-export protocol flow request `{forbidden}`"
        );
    }

    assert!(
        content.contains("pub(crate) use types::{FlowFailure, FlowStartResult, UdpCandidate};"),
        "src/runtime/udp_dispatch/mod.rs should keep only generic UDP dispatch result types in the root facade"
    );
}

#[test]
fn protocol_udp_state_manager_fields_are_not_crate_public() {
    let content = read("src/protocol_runtime/udp/state.rs");

    for field in [
        "vless",
        "vmess",
        "shadowsocks",
        "packet_path",
        "trojan",
        "mieru",
        "hysteria2",
    ] {
        assert!(
            !content.contains(&format!("pub(crate) {field}:")),
            "ProtocolUdpState manager field `{field}` should not be crate-public"
        );
    }
}

#[test]
fn protocol_udp_root_does_not_reexport_manager_internals() {
    let root = read("src/protocol_runtime/udp/mod.rs");

    for forbidden in [
        "H2ChainManager",
        "H2SendExisting",
        "MieruChainManager",
        "MieruSendExisting",
        "MieruRelayExisting",
        "SsChainManager",
        "SsSendExisting",
        "TrojanChainManager",
        "TrojanSendExisting",
        "TrojanRelayExisting",
    ] {
        assert!(
            !root.contains(forbidden),
            "src/protocol_runtime/udp/mod.rs should expose protocol UDP facades, not manager internals; found `{forbidden}`"
        );
    }
}

#[test]
fn protocol_udp_manager_roots_do_not_reexport_request_models() {
    for (source, forbidden) in [
        ("src/protocol_runtime/udp/h2_manager.rs", "H2SendExisting"),
        (
            "src/protocol_runtime/udp/mieru_manager.rs",
            "MieruSendExisting",
        ),
        (
            "src/protocol_runtime/udp/mieru_manager.rs",
            "MieruRelayExisting",
        ),
        ("src/protocol_runtime/udp/ss_manager.rs", "SsSendExisting"),
        (
            "src/protocol_runtime/udp/trojan_manager.rs",
            "TrojanSendExisting",
        ),
        (
            "src/protocol_runtime/udp/trojan_manager.rs",
            "TrojanRelayExisting",
        ),
    ] {
        let content = read(source);
        assert!(
            !content.lines().any(
                |line| line.trim_start().starts_with("pub(crate) use model::")
                    && line.contains(forbidden)
            ),
            "{source} should not re-export manager request model `{forbidden}`"
        );
    }
}

#[test]
fn udp_dispatch_cached_flow_fast_path_delegates_to_protocol_state() {
    let content = read("src/runtime/udp_dispatch/dispatch.rs");

    assert!(
        content.contains("send_existing_cached_flow"),
        "UDP dispatch should delegate cached protocol flow handling to ProtocolUdpState"
    );

    let normalized = content.replace("\r\n", "\n");
    for forbidden in [
        ".protocol_state\n            .vless",
        ".protocol_state\n            .vmess",
    ] {
        assert!(
            !normalized.contains(forbidden),
            "src/runtime/udp_dispatch/dispatch.rs should not reach into protocol manager `{forbidden}` directly"
        );
    }
}

#[test]
fn udp_relay_start_delegates_packet_path_chain_to_protocol_state() {
    let content = read("src/runtime/udp_dispatch/start/relay.rs");

    assert!(
        content.contains("send_packet_path_chain"),
        "UDP relay start should delegate packet-path manager work to ProtocolUdpState"
    );
    assert!(
        !content.contains(".packet_path"),
        "src/runtime/udp_dispatch/start/relay.rs should not reach into packet_path manager directly"
    );
    assert!(
        !content.contains("UdpFlowOutbound::"),
        "src/runtime/udp_dispatch/start/relay.rs should not construct UDP flow outbound variants directly"
    );
}

#[test]
fn udp_forward_stays_protocol_neutral_and_does_not_construct_peer_types() {
    let content = read("src/runtime/udp_dispatch/forward.rs");

    assert!(
        content.contains("forward_existing_protocol_flow"),
        "src/runtime/udp_dispatch/forward.rs should delegate protocol manager forwarding to ProtocolUdpState"
    );

    for forbidden in [
        "SsUdpPeer",
        "H2UdpPeer",
        "TrojanUdpPeer",
        "MieruUdpPeer",
        "UdpPeerEndpoint",
        "Socks5UdpSend",
        "protocol_runtime::socks5_udp",
        ".packet_path",
        ".shadowsocks",
        ".hysteria2",
        ".trojan",
        ".mieru",
        "UdpFlowOutbound::Direct",
        "UdpFlowOutbound::Socks5",
        "UdpFlowOutbound::Shadowsocks",
        "UdpFlowOutbound::Hysteria2",
        "UdpFlowOutbound::Trojan",
        "UdpFlowOutbound::Mieru",
    ] {
        assert!(
            !content.contains(forbidden),
            "src/runtime/udp_dispatch/forward.rs should not construct protocol peer types; found `{forbidden}`"
        );
    }
}

#[test]
fn protocol_udp_existing_flow_forward_lives_outside_state_root() {
    let state = read("src/protocol_runtime/udp/state.rs");
    let forward = manifest_dir().join("src/protocol_runtime/udp/state/forward.rs");

    for forbidden in [
        "fn forward_existing_protocol_flow",
        "UdpFlowOutbound::Hysteria2",
        "UdpFlowOutbound::Trojan",
        "UdpFlowOutbound::Mieru",
        "UdpFlowOutbound::Direct",
        "UdpFlowOutbound::Socks5",
        "udp_protocol_forward",
    ] {
        assert!(
            !state.contains(forbidden),
            "src/protocol_runtime/udp/state.rs should keep existing-flow forwarding details in state/forward.rs; found `{forbidden}`"
        );
    }
    assert!(
        forward.exists(),
        "existing UDP protocol-flow forwarding should live in protocol_runtime/udp/state/forward.rs"
    );
}

#[test]
fn protocol_udp_existing_flow_handlers_live_outside_forward_dispatch() {
    let forward = read("src/protocol_runtime/udp/state/forward.rs");
    let root = manifest_dir().join("src/protocol_runtime/udp/state/forward");

    for forbidden in [
        "SsSendExisting",
        "H2SendExisting",
        "TrojanSendExisting",
        "MieruSendExisting",
        "ExistingFlow {",
        "ProtocolUdpFlowSnapshot::Shadowsocks",
        "ProtocolUdpFlowSnapshot::Hysteria2",
        "ProtocolUdpFlowSnapshot::Trojan",
        "ProtocolUdpFlowSnapshot::Mieru",
        "datagram_cache_key",
        "cipher_kind",
        "client_fingerprint",
        "relay_chain",
        ".upstream()",
    ] {
        assert!(
            !forward.contains(forbidden),
            "state/forward.rs should delegate protocol UDP flow field extraction to state/forward/*.rs; found `{forbidden}`"
        );
    }
    for path in ["shadowsocks.rs", "hysteria2.rs", "trojan.rs", "mieru.rs"] {
        assert!(
            root.join(path).exists(),
            "existing UDP protocol-flow handler should live in state/forward/{path}"
        );
    }
}

#[test]
fn protocol_udp_cached_flow_fast_path_lives_outside_state_root() {
    let state = read("src/protocol_runtime/udp/state.rs");
    let cached = manifest_dir().join("src/protocol_runtime/udp/state/cached.rs");

    for forbidden in [
        "fn send_existing_cached_flow",
        ".vless\n            .send_existing",
        ".vmess\n            .send_existing",
    ] {
        assert!(
            !state.contains(forbidden),
            "src/protocol_runtime/udp/state.rs should keep cached-flow forwarding details in state/cached.rs; found `{forbidden}`"
        );
    }
    assert!(
        cached.exists(),
        "cached UDP flow forwarding should live in protocol_runtime/udp/state/cached.rs"
    );
}

#[test]
fn protocol_udp_packet_path_facade_lives_outside_state_root() {
    let state = read("src/protocol_runtime/udp/state.rs");
    let packet_path_content = read("src/protocol_runtime/udp/state/packet_path.rs");
    let packet_path = manifest_dir().join("src/protocol_runtime/udp/state/packet_path.rs");

    for forbidden in [
        "fn datagram_chain_flow_outbound",
        "fn send_packet_path_chain",
        "UdpFlowOutbound::Shadowsocks",
        "packet_path_carrier",
    ] {
        assert!(
            !state.contains(forbidden),
            "src/protocol_runtime/udp/state.rs should keep packet-path facade details in state/packet_path.rs; found `{forbidden}`"
        );
    }
    assert!(
        packet_path.exists(),
        "UDP packet-path facade should live in protocol_runtime/udp/state/packet_path.rs"
    );
    for forbidden in [
        "ProtocolUdpFlowSnapshot::Shadowsocks",
        "ProtocolUdpFlowSnapshot",
        "password: datagram.password",
        "cipher_kind: datagram.cipher_kind",
        "datagram_cache_key: datagram.datagram_cache_key",
        ".into_protocol_snapshot()",
        ".with_packet_path_carrier(",
    ] {
        assert!(
            !packet_path_content.contains(forbidden),
            "packet-path state should consume the datagram source protocol snapshot instead of constructing Shadowsocks snapshots directly; found `{forbidden}`"
        );
    }
    assert!(
        packet_path_content.contains("UdpFlowOutbound::PacketPathDatagram")
            && packet_path_content.contains("flow_binding.into_parts()")
            && packet_path_content.contains("snapshot: flow_snapshot"),
        "packet-path state should store a neutral packet-path flow snapshot without converting it to a protocol UDP snapshot"
    );
}

#[test]
fn adapters_do_not_construct_udp_dispatch_peer_helpers() {
    for path in rust_sources_under("src/adapters") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        for forbidden in [
            "SsUdpPeer",
            "H2UdpPeer",
            "TrojanUdpPeer",
            "MieruUdpPeer",
            "UdpFlowContext",
            "UdpPacketRef",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should not construct udp-dispatch peer helper `{forbidden}`"
            );
        }
    }
}

#[test]
fn packet_path_chain_does_not_own_socks5_runtime_state() {
    let content = read("src/protocol_runtime/udp/packet_path_chain.rs");

    for forbidden in [
        "ActiveUpstreamSocks5UdpAssociation",
        "Socks5PacketPath",
        "socks5::parse_udp_packet",
    ] {
        assert!(
            !content.contains(forbidden),
            "src/protocol_runtime/udp/packet_path_chain.rs should stay generic; found `{forbidden}`"
        );
    }
}

#[test]
fn packet_path_traits_are_grouped_by_responsibility() {
    let facade = read("src/protocol_runtime/udp/packet_path_traits.rs");
    let carrier = read("src/protocol_runtime/udp/packet_path_traits/carrier.rs");
    let root = manifest_dir().join("src/protocol_runtime/udp/packet_path_traits");
    let peer = manifest_dir().join("src/protocol_runtime/udp/peer.rs");

    for forbidden in [
        "trait PacketPathCarrier",
        "struct PacketPathCarrierDescriptor",
        "struct UdpDatagramSource",
        "type ChainTask =",
        "struct UdpFlowContext",
        "struct UdpPacketRef",
        "struct SsUdpPeer",
        "struct H2UdpPeer",
        "struct TrojanUdpPeer",
        "struct MieruUdpPeer",
        "mod peer",
        "peer::",
    ] {
        assert!(
            !facade.contains(forbidden),
            "packet_path_traits.rs should stay a facade and keep grouped definitions in packet_path_traits/*.rs; found `{forbidden}`"
        );
    }
    for path in ["carrier.rs", "context.rs"] {
        assert!(
            root.join(path).exists(),
            "packet-path trait/helper definitions should keep grouped module packet_path_traits/{path}"
        );
    }
    assert!(
        peer.exists() && !root.join("peer.rs").exists(),
        "protocol UDP peer models should live outside packet_path_traits"
    );
    assert!(
        !carrier.contains("ProtocolAdapter::"),
        "packet-path trait docs should not describe packet-path products as monolithic ProtocolAdapter outputs"
    );
    assert!(
        carrier.contains("UdpPacketPathCapability::udp_packet_path_carrier_descriptor")
            && carrier.contains("UdpPacketPathCapability::udp_datagram_source"),
        "packet-path trait docs should point carrier/datagram products at UdpPacketPathCapability"
    );
}

#[test]
fn packet_path_carriers_live_outside_chain_manager() {
    let manager = read("src/protocol_runtime/udp/packet_path_chain.rs");
    let carriers = manifest_dir().join("src/protocol_runtime/udp/packet_path_chain/carriers.rs");

    for forbidden in ["struct ShadowsocksPacketPath", "struct Hysteria2PacketPath"] {
        assert!(
            !manager.contains(forbidden),
            "packet_path_chain.rs should keep concrete carrier implementations in carriers.rs; found `{forbidden}`"
        );
    }
    assert!(
        carriers.exists(),
        "packet-path carrier implementations should live in packet_path_chain/carriers.rs"
    );
}

#[test]
fn packet_path_protocol_carriers_live_outside_carrier_facade() {
    let facade = read("src/protocol_runtime/udp/packet_path_chain/carriers.rs");
    let udp_socket = manifest_dir()
        .join("src/protocol_runtime/udp/packet_path_chain/carriers/udp_socket_carrier.rs");
    let quic_datagram = manifest_dir()
        .join("src/protocol_runtime/udp/packet_path_chain/carriers/quic_datagram_carrier.rs");

    for forbidden in [
        "struct ShadowsocksPacketPath",
        "struct Hysteria2PacketPath",
        "shadowsocks_carrier",
        "hysteria2_carrier",
        "ShadowsocksDatagramCodec",
        "Hysteria2UdpPacketTarget",
        r#"#[cfg(feature = "hysteria2")]"#,
        "connect_raw",
        "build_shadowsocks_packet_path",
        "build_hysteria2_packet_path",
    ] {
        assert!(
            !facade.contains(forbidden),
            "packet_path_chain/carriers.rs should keep concrete carrier internals in protocol files; found `{forbidden}`"
        );
    }
    assert!(
        udp_socket.exists(),
        "UDP socket packet-path carrier should live in carriers/udp_socket_carrier.rs"
    );
    assert!(
        quic_datagram.exists(),
        "QUIC datagram packet-path carrier should live in carriers/quic_datagram_carrier.rs"
    );
}

#[test]
fn packet_path_chain_root_does_not_reexport_protocol_carrier_builders() {
    let root = read("src/protocol_runtime/udp/packet_path_chain.rs");

    for forbidden in [
        "pub(crate) use carriers::build_shadowsocks_packet_path",
        "pub(crate) use carriers::build_hysteria2_packet_path",
    ] {
        assert!(
            !root.contains(forbidden),
            "packet_path_chain.rs should not re-export protocol carrier builder `{forbidden}`"
        );
    }
    assert!(
        root.contains("pub(crate) mod carriers;"),
        "packet_path_chain.rs should expose the explicit carriers module for adapter capability bridges"
    );

    for source in [
        "src/adapters/shadowsocks/udp.rs",
        "src/adapters/hysteria2/udp.rs",
    ] {
        let content = read(source);
        assert!(
            content.contains("packet_path_chain::carriers::")
                && content.contains("_carrier::build("),
            "{source} should call packet-path carrier builders through the explicit carrier submodule"
        );
    }
}

#[test]
fn packet_path_response_bridge_lives_outside_chain_manager() {
    let manager = read("src/protocol_runtime/udp/packet_path_chain.rs");
    let bridge = manifest_dir().join("src/protocol_runtime/udp/packet_path_chain/bridge.rs");

    for forbidden in ["async fn recv_loop", "fn remove_waiter"] {
        assert!(
            !manager.contains(forbidden),
            "packet_path_chain.rs should keep response bridge internals in bridge.rs; found `{forbidden}`"
        );
    }
    assert!(
        bridge.exists(),
        "packet-path response bridge should live in packet_path_chain/bridge.rs"
    );
}

#[test]
fn packet_path_key_model_lives_outside_chain_manager() {
    let manager = read("src/protocol_runtime/udp/packet_path_chain.rs");
    let key = manifest_dir().join("src/protocol_runtime/udp/packet_path_chain/key.rs");
    let key_content = read("src/protocol_runtime/udp/packet_path_chain/key.rs");
    let model = read("src/protocol_runtime/udp/packet_path_chain/model.rs");
    let traits = read("src/protocol_runtime/udp/packet_path_traits/carrier.rs");

    for forbidden in [
        "struct PathKey",
        "carrier_key: carrier.cache_key().to_owned()",
        "datagram_cache_key: datagram_cache_key.to_owned()",
    ] {
        assert!(
            !manager.contains(forbidden),
            "packet_path_chain.rs should keep key construction details in packet_path_chain/key.rs; found `{forbidden}`"
        );
    }
    assert!(
        key.exists(),
        "packet-path key model should live in packet_path_chain/key.rs"
    );
    assert!(
        !key_content.contains("UdpDatagramSource")
            && !key_content.contains("datagram.datagram_cache_key")
            && !key_content.contains("UdpPacketPathCarrier"),
        "packet-path key model should use opaque carrier/datagram key parts instead of reading source internals"
    );
    assert!(
        model.contains("self.datagram.descriptor().key_part()")
            && traits.contains("struct UdpDatagramKey")
            && traits.contains("fn key_part(&self) -> UdpDatagramKey"),
        "UdpDatagramSource should expose a neutral descriptor that provides the packet-path datagram key part"
    );
}

#[test]
fn packet_path_entry_model_lives_outside_chain_manager() {
    let manager = read("src/protocol_runtime/udp/packet_path_chain.rs");
    let model = read("src/protocol_runtime/udp/packet_path_chain/model.rs");

    for forbidden in [
        "struct Entry",
        "struct EntryCandidate",
        "fn key(&self) -> PathKey",
    ] {
        assert!(
            !manager.contains(forbidden),
            "packet_path_chain.rs should keep entry model details in packet_path_chain/model.rs; found `{forbidden}`"
        );
    }

    for required in [
        "struct Entry",
        "struct EntryCandidate",
        "fn key(&self) -> PathKey",
    ] {
        assert!(
            model.contains(required),
            "packet-path entry model should live in packet_path_chain/model.rs; missing `{required}`"
        );
    }
}

#[test]
fn packet_path_entry_build_lives_outside_chain_manager() {
    let manager = read("src/protocol_runtime/udp/packet_path_chain.rs");
    let entry_content = read("src/protocol_runtime/udp/packet_path_chain/entry.rs");
    let entry = manifest_dir().join("src/protocol_runtime/udp/packet_path_chain/entry.rs");

    for forbidden in [
        "udp_packet_path_carrier_descriptor",
        "udp_datagram_source",
        "ShadowsocksDatagramCodec",
        "tokio::spawn(recv_loop",
    ] {
        assert!(
            !manager.contains(forbidden),
            "packet_path_chain.rs should keep entry build details in packet_path_chain/entry.rs; found `{forbidden}`"
        );
    }
    assert!(
        entry.exists(),
        "packet-path entry build logic should live in packet_path_chain/entry.rs"
    );
    assert!(
        !entry_content.contains("ShadowsocksDatagramCodec"),
        "packet-path entry build should use adapter-provided datagram codecs instead of constructing Shadowsocks codec directly"
    );
    assert!(
        entry_content.contains("candidate.datagram.codec.clone()"),
        "packet-path entry build should clone the codec supplied by UdpDatagramSource"
    );
}

#[test]
fn packet_path_diagnostics_live_outside_chain_manager() {
    let manager = read("src/protocol_runtime/udp/packet_path_chain.rs");
    let diagnostics =
        manifest_dir().join("src/protocol_runtime/udp/packet_path_chain/diagnostics.rs");

    for forbidden in ["fn carrier_upstream", "orchestration::endpoint"] {
        assert!(
            !manager.contains(forbidden),
            "packet_path_chain.rs should keep diagnostics helpers in packet_path_chain/diagnostics.rs; found `{forbidden}`"
        );
    }
    assert!(
        diagnostics.exists(),
        "packet-path diagnostics helpers should live in packet_path_chain/diagnostics.rs"
    );
}

#[test]
fn packet_path_snapshot_lookup_lives_outside_chain_manager() {
    let manager = read("src/protocol_runtime/udp/packet_path_chain.rs");
    let snapshot_content = read("src/protocol_runtime/udp/packet_path_chain/snapshot.rs");
    let snapshot = manifest_dir().join("src/protocol_runtime/udp/packet_path_chain/snapshot.rs");

    for forbidden in [
        "PathKey::from_lookup",
        "packet_path_carrier_dropped",
        "cached packet-path carrier not found",
    ] {
        assert!(
            !manager.contains(forbidden),
            "packet_path_chain.rs should keep snapshot lookup details in packet_path_chain/snapshot.rs; found `{forbidden}`"
        );
    }
    assert!(
        snapshot.exists(),
        "packet-path snapshot lookup should live in packet_path_chain/snapshot.rs"
    );
    assert!(
        snapshot_content.contains("lookup_key: PacketPathLookupKey")
            && !snapshot_content.contains("PacketPathFlowSnapshot")
            && !snapshot_content.contains("UdpPacketPathCarrier"),
        "packet-path snapshot lookup should receive a neutral packet-path lookup key"
    );
}

#[test]
fn packet_path_snapshot_send_uses_request_model() {
    let manager = read("src/protocol_runtime/udp/packet_path_chain.rs");
    let packet_path = read("src/protocol_runtime/udp/state/packet_path.rs");

    assert!(
        manager.contains("struct SendWithSnapshotRequest")
            && manager.contains("request: SendWithSnapshotRequest<'_>")
            && manager.contains("lookup_key: PacketPathLookupKey"),
        "packet-path snapshot send should use a request model"
    );
    assert!(
        packet_path.contains("SendWithSnapshotRequest {")
            && packet_path.contains("lookup_key: snapshot.lookup_key()")
            && !packet_path.contains("carrier_cache_key: &snapshot.carrier_cache_key")
            && !packet_path.contains("datagram_cache_key: &snapshot.datagram_cache_key")
            && packet_path.contains("pub(crate) async fn forward_existing_packet_path_flow"),
        "packet-path snapshot forward path should convert snapshots into neutral lookup keys without unpacking cache fields"
    );
}

#[test]
fn feature_gated_udp_manager_modules_do_not_embed_disabled_stubs() {
    for source in [
        "src/protocol_runtime/udp/h2_manager.rs",
        "src/protocol_runtime/udp/mieru_manager.rs",
        "src/protocol_runtime/udp/trojan_manager.rs",
    ] {
        let content = read(source);
        assert!(
            !content.contains("#[cfg(not(feature ="),
            "{source} should not mix enabled manager logic with disabled-feature stubs"
        );
    }
}

#[test]
fn trojan_udp_socket_wrappers_live_outside_manager() {
    let manager = read("src/protocol_runtime/udp/trojan_manager.rs");
    let socket = manifest_dir().join("src/protocol_runtime/udp/trojan_manager/socket.rs");

    for forbidden in ["struct ReadOnlySocket", "struct WriteOnlySocket"] {
        assert!(
            !manager.contains(forbidden),
            "trojan_manager.rs should keep stream socket adapters in trojan_manager/socket.rs; found `{forbidden}`"
        );
    }
    assert!(
        socket.exists(),
        "Trojan UDP socket wrappers should live in trojan_manager/socket.rs"
    );
}

#[test]
fn trojan_udp_response_bridge_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/trojan_manager.rs");
    let bridge = manifest_dir().join("src/protocol_runtime/udp/trojan_manager/bridge.rs");

    for forbidden in [
        "broadcast::channel",
        "recv_tx.subscribe",
        "fn spawn_bridge",
        "trojan upstream closed",
    ] {
        assert!(
            !manager.contains(forbidden),
            "trojan_manager.rs should keep response bridge details in trojan_manager/bridge.rs; found `{forbidden}`"
        );
    }
    assert!(
        bridge.exists(),
        "Trojan UDP response bridge should live in trojan_manager/bridge.rs"
    );
}

#[test]
fn trojan_udp_tls_connect_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/trojan_manager.rs");
    let connect = manifest_dir().join("src/protocol_runtime/udp/trojan_manager/connect.rs");

    for forbidden in [
        "ClientTlsConfig",
        "connect_tls_upstream",
        "connect_tls_stream",
        ".connect_host(",
    ] {
        assert!(
            !manager.contains(forbidden),
            "trojan_manager.rs should keep TLS connect details in trojan_manager/connect.rs; found `{forbidden}`"
        );
    }
    assert!(
        connect.exists(),
        "Trojan UDP TLS connect helpers should live in trojan_manager/connect.rs"
    );
}

#[test]
fn trojan_udp_flow_resume_is_protocol_owned() {
    let adapter = read("src/adapters/trojan/udp.rs");
    let snapshot = read("src/protocol_runtime/udp/flow_snapshot.rs");
    let forward = read("src/protocol_runtime/udp/state/forward/trojan.rs");
    let start = read("src/protocol_runtime/udp/start/trojan.rs");
    let manager_send = read("src/protocol_runtime/udp/trojan_manager/send.rs");
    let manager_connect = read("src/protocol_runtime/udp/trojan_manager/connect.rs");
    let manager_establish = read("src/protocol_runtime/udp/trojan_manager/establish.rs");
    let manager_stream = read("src/protocol_runtime/udp/trojan_manager/stream.rs");
    let manager_model = read("src/protocol_runtime/udp/trojan_manager/model.rs");
    let protocol_outbound =
        fs::read_to_string(repo_root().join("protocols/trojan/src/outbound.rs"))
            .expect("read trojan protocol outbound source");

    assert!(
        adapter.contains("TrojanUdpFlowResume::new")
            && protocol_outbound.contains("struct TrojanUdpFlowResume")
            && protocol_outbound.contains("pub fn peer_config(&self)")
            && protocol_outbound.contains("pub fn flow_key(&self")
            && protocol_outbound.contains("struct TrojanUdpTlsProfile")
            && protocol_outbound.contains("pub fn tls_profile(&self")
            && protocol_outbound.contains("pub async fn establish_udp_tunnel")
            && protocol_outbound.contains("struct TrojanUdpLeafKey")
            && protocol_outbound.contains("pub fn client_fingerprint(&self) -> Option<&str>")
            && protocol_outbound.contains("pub fn relay_chain(&self) -> bool"),
        "Trojan adapter should build an opaque protocol-owned UDP flow resume descriptor"
    );
    assert!(
        snapshot.contains("resume: ProtocolUdpFlowResume")
            && snapshot.contains("Trojan(trojan::TrojanUdpFlowResume)")
            && !snapshot.contains("password: String")
            && !snapshot.contains("client_fingerprint: Option<String>")
            && !snapshot.contains("relay_chain: bool"),
        "Trojan protocol UDP flow snapshot should carry only the unified opaque resume wrapper"
    );
    assert!(
        forward.contains("resume: existing.resume.clone()")
            && !forward.contains("existing.resume.password()")
            && !forward.contains("existing.resume.sni()")
            && !forward.contains("existing.resume.insecure()")
            && !forward.contains("existing.resume.client_fingerprint()")
            && !forward.contains("existing.resume.relay_chain()")
            && !forward.contains("password: &'a str")
            && !forward.contains("client_fingerprint: Option<&'a str>")
            && !forward.contains("relay_chain: bool"),
        "existing Trojan UDP flow forwarding should pass the opaque resume descriptor without unpacking auth, TLS, or relay state"
    );
    assert!(
        start.contains("ProtocolUdpFlowResume::Trojan(resume)")
            && start.contains("resume: resume.clone()")
            && !start.contains("resume.password()")
            && !start.contains("resume.sni()")
            && !start.contains("resume.insecure()")
            && !start.contains("resume.client_fingerprint()")
            && !start.contains("resume.relay_chain()"),
        "new Trojan UDP flow start should pass the opaque resume descriptor without unpacking auth, TLS, or relay state"
    );
    for forbidden in [
        "request.resume.password()",
        "request.resume.sni()",
        "request.resume.insecure()",
        "request.resume.client_fingerprint()",
        "request.resume.relay_chain()",
        ".peer_config()",
        "peer_config.",
        "peer_config:",
        "TrojanUdpPeerConfig",
        "TrojanKey::Leaf {",
        "password: String",
    ] {
        assert!(
            !manager_send.contains(forbidden)
                && !manager_connect.contains(forbidden)
                && !manager_establish.contains(forbidden)
                && !manager_stream.contains(forbidden)
                && !manager_model.contains(forbidden),
            "Trojan UDP manager should use protocol-owned peer config/key instead of unpacking `{forbidden}`"
        );
    }
    assert!(
        manager_send.contains("request.resume.flow_key(request.server, request.port)")
            && manager_connect.contains("peer.resume.tls_profile(")
            && manager_stream.contains(".establish_with_resume("),
        "Trojan UDP manager should consume protocol-owned flow key, TLS profile, and tunnel establishment helpers"
    );
}

#[test]
fn trojan_udp_packet_stream_tasks_live_outside_manager() {
    let manager = read("src/protocol_runtime/udp/trojan_manager.rs");
    let stream = read("src/protocol_runtime/udp/trojan_manager/stream.rs");
    let model = read("src/protocol_runtime/udp/trojan_manager/model.rs");

    for forbidden in ["MeteredStream", "tokio::io::split"] {
        assert!(
            !manager.contains(forbidden),
            "trojan_manager.rs should keep packet stream task details in trojan_manager/stream.rs; found `{forbidden}`"
        );
    }
    for forbidden in [
        "UdpPacketStreamFraming",
        "write_udp_packet",
        "read_udp_packet",
        "establish_udp_packet_tunnel",
        "read_udp_flow_packet",
        "write_udp_flow_packet",
        "TrojanUdpPacket {",
    ] {
        assert!(
            !manager.contains(forbidden),
            "trojan_manager.rs should not own Trojan packet framing details; found `{forbidden}`"
        );
        assert!(
            !stream.contains(forbidden),
            "trojan_manager/stream.rs should delegate Trojan packet framing to protocols/trojan helpers; found `{forbidden}`"
        );
        assert!(
            !model.contains(forbidden),
            "trojan_manager/model.rs should not rebuild Trojan packet framing details; found `{forbidden}`"
        );
    }
    assert!(
        !stream.contains("trojan::write_udp_response")
            && !stream.contains("trojan::read_inbound_udp_packet"),
        "Trojan UDP manager stream should use flow-specific protocol helpers instead of generic UDP helpers"
    );
    assert!(
        stream.contains("trojan::TrojanUdpFlowIo")
            && stream.contains("trojan::TrojanUdpPacket")
            && stream.contains("flow_io\n                .write_stream_packet")
            && stream.contains("flow_io.read_stream_packet(&mut recv_stream)")
            && !stream.contains(".write_packet")
            && !stream.contains(".read_packet")
            && !stream.contains("packet.write_to")
            && !stream.contains("packet.target")
            && !stream.contains("packet.payload")
            && !model.contains("struct TrojanPacket"),
        "Trojan UDP packet stream tasks should use protocol-owned stream operations instead of unpacking packet fields"
    );
}

#[test]
fn mieru_udp_packet_codec_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/mieru_manager.rs");
    let stream = read("src/protocol_runtime/udp/mieru_manager/stream.rs");
    let connect = read("src/protocol_runtime/udp/mieru_manager/connect.rs");
    let adapter = read("src/adapters/mieru/udp.rs");
    let snapshot = read("src/protocol_runtime/udp/flow_snapshot.rs");
    let forward = read("src/protocol_runtime/udp/state/forward/mieru.rs");
    let manager_send = read("src/protocol_runtime/udp/mieru_manager/send.rs");
    let manager_connect = read("src/protocol_runtime/udp/mieru_manager/connect.rs");
    let manager_establish = read("src/protocol_runtime/udp/mieru_manager/establish.rs");
    let manager_model = read("src/protocol_runtime/udp/mieru_manager/model.rs");
    let protocol_udp = fs::read_to_string(repo_root().join("protocols/mieru/src/udp.rs"))
        .expect("read mieru protocol udp source");
    let protocol_outbound = fs::read_to_string(repo_root().join("protocols/mieru/src/outbound.rs"))
        .expect("read mieru protocol outbound source");

    for forbidden in [
        "UdpPacketFraming",
        "MieruUdpAssociatePacket",
        "MieruInboundUdpPacket",
        "fn encode_associate_packet",
        "fn decode_associate_packet",
        "socks5::build_udp_packet",
        "socks5::parse_udp_packet",
    ] {
        assert!(
            !manager.contains(forbidden),
            "mieru_manager.rs should not own Mieru associate packet codec details; found `{forbidden}`"
        );
        assert!(
            !stream.contains(forbidden),
            "mieru_manager/stream.rs should delegate Mieru packet codec details to protocols/mieru; found `{forbidden}`"
        );
    }
    assert!(
        !manifest_dir()
            .join("src/protocol_runtime/udp/mieru_manager/codec.rs")
            .exists(),
        "Mieru UDP manager should not keep a proxy-owned codec module"
    );
    assert!(
        protocol_outbound.contains("struct MieruUdpFlowIo")
            && protocol_outbound.contains("struct MieruUdpFlowPacket")
            && protocol_outbound.contains("encode_udp_flow_packet")
            && protocol_outbound.contains("decode_udp_flow_packet")
            && protocol_outbound.contains("encrypt_payload")
            && protocol_outbound.contains("next_packet")
            && connect.contains("MieruUdpFlowIo::establish"),
        "Mieru UDP flow associate, encryption, and packet codec should live behind a protocol-owned flow I/O helper"
    );
    assert!(
        !manager_model.contains("struct MieruPacket")
            && manager_model.contains("mieru::MieruUdpFlowPacket")
            && manager_send.contains("MieruUdpFlowPacket::new")
            && stream.contains("MieruUdpFlowPacket")
            && stream.contains("io.write_packet(&mut write_stream, &packet)")
            && stream.contains("io.read_packets(&mut read_stream, &mut scratch)")
            && stream.contains("packet.into_parts()")
            && !stream.contains("packet.target")
            && !stream.contains("packet.payload"),
        "Mieru UDP manager should use protocol-owned UDP flow stream operations instead of unpacking packet fields in proxy"
    );
    assert!(
        protocol_udp.contains("pub fn udp_flow_codec(")
            && protocol_udp.contains("impl DatagramCodec<Address> for MieruUdpFlowCodec")
            && !adapter.contains("mieru::udp_flow_codec"),
        "Mieru ordinary UDP flow codec should be protocol-owned and recovered from the resume descriptor, not carried by the adapter"
    );
    assert!(
        adapter.contains("MieruUdpFlowResume::new")
            && protocol_udp.contains("struct MieruUdpFlowResume")
            && protocol_udp.contains("pub fn peer_config(&self)")
            && protocol_udp.contains("pub fn flow_key(&self")
            && protocol_udp.contains("enum MieruUdpFlowKey")
            && protocol_udp.contains("struct MieruUdpLeafKey")
            && protocol_udp.contains("pub fn codec(&self)")
            && protocol_udp.contains("pub fn relay_chain(&self) -> bool"),
        "Mieru adapter should build an opaque protocol-owned UDP flow resume descriptor"
    );
    assert!(
        snapshot.contains("resume: ProtocolUdpFlowResume")
            && snapshot.contains("Mieru(mieru::MieruUdpFlowResume)")
            && !snapshot.contains("username: String")
            && !snapshot.contains("relay_chain: bool"),
        "Mieru protocol UDP flow snapshot should carry only the unified opaque resume wrapper"
    );
    assert!(
        forward.contains("resume: existing.resume.clone()")
            && !forward.contains("existing.resume.username()")
            && !forward.contains("existing.resume.password()")
            && !forward.contains("existing.resume.relay_chain()")
            && !forward.contains("existing.resume.codec()")
            && !forward.contains("mieru::udp_flow_codec")
            && !forward.contains("username: &'a str")
            && !forward.contains("relay_chain: bool"),
        "existing Mieru UDP flow forwarding should pass the opaque resume descriptor without unpacking account or relay state"
    );
    let start = read("src/protocol_runtime/udp/start/mieru.rs");
    assert!(
        start.contains("ProtocolUdpFlowResume::Mieru(resume)")
            && start.contains("resume: resume.clone()")
            && !start.contains("resume.username()")
            && !start.contains("resume.password()")
            && !start.contains("resume.relay_chain()")
            && !start.contains("resume.codec()"),
        "new Mieru UDP flow start should pass the opaque resume descriptor without unpacking account or relay state"
    );
    for forbidden in [
        "request.resume.username()",
        "request.resume.password()",
        "request.resume.relay_chain()",
        ".peer_config()",
        "peer_config.",
        "peer_config:",
        "MieruUdpPeerConfig",
        "MieruKey::Leaf {",
        "username: String",
        "password: String",
    ] {
        assert!(
            !manager_send.contains(forbidden)
                && !manager_connect.contains(forbidden)
                && !manager_establish.contains(forbidden)
                && !manager_model.contains(forbidden),
            "Mieru UDP manager should use protocol-owned peer config/key instead of unpacking `{forbidden}`"
        );
    }
    assert!(
        manager_send.contains("request.resume.flow_key(request.server, request.port)")
            && manager_connect.contains("MieruUdpFlowIo::establish_with_resume")
            && manager_establish.contains("packet_stream(stream, peer.resume).await")
            && protocol_outbound.contains("pub async fn establish_with_resume"),
        "Mieru UDP manager should consume protocol-owned flow key and UDP establish helper"
    );
}

#[test]
fn mieru_udp_response_bridge_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/mieru_manager.rs");
    let bridge = manifest_dir().join("src/protocol_runtime/udp/mieru_manager/bridge.rs");

    for forbidden in [
        "type RecvItem",
        "broadcast::channel",
        "recv_tx.subscribe",
        "fn spawn_bridge",
        "mieru upstream closed",
    ] {
        assert!(
            !manager.contains(forbidden),
            "mieru_manager.rs should keep response bridge details in mieru_manager/bridge.rs; found `{forbidden}`"
        );
    }
    assert!(
        bridge.exists(),
        "Mieru UDP response bridge should live in mieru_manager/bridge.rs"
    );
}

#[test]
fn mieru_udp_connect_handshake_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/mieru_manager.rs");
    let connect = read("src/protocol_runtime/udp/mieru_manager/connect.rs");
    let protocol_outbound = fs::read_to_string(repo_root().join("protocols/mieru/src/outbound.rs"))
        .expect("read mieru protocol outbound source");

    for forbidden in [
        "MieruOutbound::connect",
        ".connect_host(",
        "ASSOCIATE",
        "encrypt_client_data(&assoc_req)",
        "mieru udp assoc",
    ] {
        assert!(
            !manager.contains(forbidden),
            "mieru_manager.rs should keep connect and UDP associate handshake details in mieru_manager/connect.rs; found `{forbidden}`"
        );
    }
    assert!(
        connect.contains("MieruUdpFlowIo::establish")
            && !connect.contains("MieruOutbound::connect")
            && !connect.contains("encrypt_client_data")
            && !connect.contains("decrypt_server_data")
            && protocol_outbound.contains("fn send_udp_associate_request")
            && protocol_outbound.contains("fn read_udp_associate_response"),
        "Mieru UDP associate handshake should live behind protocols/mieru flow I/O"
    );
}

#[test]
fn mieru_udp_state_model_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/mieru_manager.rs");
    let model = manifest_dir().join("src/protocol_runtime/udp/mieru_manager/model.rs");

    for forbidden in [
        "enum MieruKey",
        "struct MieruEntry",
        "struct MieruSendExisting",
        "struct MieruRelayExisting",
    ] {
        assert!(
            !manager.contains(forbidden),
            "mieru_manager.rs should keep state/request models in mieru_manager/model.rs; found `{forbidden}`"
        );
    }
    assert!(
        model.exists(),
        "Mieru UDP state/request models should live in mieru_manager/model.rs"
    );
}

#[test]
fn mieru_udp_establish_logic_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/mieru_manager.rs");
    let establish = manifest_dir().join("src/protocol_runtime/udp/mieru_manager/establish.rs");

    for forbidden in [
        "fn establish_direct",
        "fn establish_packet_stream",
        "connect::direct_stream",
        "connect::open_udp_flow",
        "spawn_packet_stream",
    ] {
        assert!(
            !manager.contains(forbidden),
            "mieru_manager.rs should keep UDP establish glue in mieru_manager/establish.rs; found `{forbidden}`"
        );
    }
    assert!(
        establish.exists(),
        "Mieru UDP establish glue should live in mieru_manager/establish.rs"
    );
}

#[test]
fn mieru_udp_send_orchestration_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/mieru_manager.rs");
    let send = manifest_dir().join("src/protocol_runtime/udp/mieru_manager/send.rs");

    for forbidden in [
        "async fn send(",
        "fn send_relay",
        "send_existing(",
        "send_relay_existing(",
        "mieru_relay_upstream",
        "mieru_establish",
        "mieru_relay_establish",
        "UdpFlowContext",
        "UdpPacketRef",
    ] {
        assert!(
            !manager.contains(forbidden),
            "mieru_manager.rs should keep send orchestration in mieru_manager/send.rs; found `{forbidden}`"
        );
    }
    assert!(
        send.exists(),
        "Mieru UDP send orchestration should live in mieru_manager/send.rs"
    );
}

#[test]
fn trojan_udp_state_model_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/trojan_manager.rs");
    let model = read("src/protocol_runtime/udp/trojan_manager/model.rs");

    for forbidden in [
        "enum TrojanKey",
        "struct TrojanEntry",
        "struct TrojanSendExisting",
        "struct TrojanRelaySend",
        "struct TrojanRelayExisting",
    ] {
        assert!(
            !manager.contains(forbidden),
            "trojan_manager.rs should keep state/request models in trojan_manager/model.rs; found `{forbidden}`"
        );
    }
    assert!(
        !model.contains("struct TrojanPacket") && model.contains("trojan::TrojanUdpPacket"),
        "Trojan UDP state/request models should use protocol-owned UDP flow packet models instead of duplicating packet shape in proxy"
    );
}

#[test]
fn trojan_udp_establish_logic_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/trojan_manager.rs");
    let establish = read("src/protocol_runtime/udp/trojan_manager/establish.rs");
    let stream = read("src/protocol_runtime/udp/trojan_manager/stream.rs");

    for forbidden in [
        "fn establish_direct",
        "fn establish_over_relay_stream",
        "fn establish_packet_stream",
        "connect::direct_tls_stream",
        "connect::relay_tls_stream",
        "spawn_packet_stream",
    ] {
        assert!(
            !manager.contains(forbidden),
            "trojan_manager.rs should keep UDP establish glue in trojan_manager/establish.rs; found `{forbidden}`"
        );
    }
    for forbidden in [
        "TrojanUdpPacket {\n        target:",
        "TrojanUdpPacketTunnelTarget",
        "UdpPacketTunnelProtocol",
    ] {
        assert!(
            !manager.contains(forbidden),
            "trojan_manager.rs should not use protocol packet structs; found `{forbidden}`"
        );
        assert!(
            !establish.contains(forbidden),
            "trojan_manager/establish.rs should use protocol-owned packet helpers without rebuilding protocol packet internals; found `{forbidden}`"
        );
        assert!(
            !stream.contains(forbidden),
            "trojan_manager/stream.rs should delegate Trojan packet tunnel establishment to protocols/trojan helpers; found `{forbidden}`"
        );
    }
    assert!(
        establish.contains("trojan::TrojanUdpPacket::new"),
        "Trojan UDP establish glue should build protocol-owned UDP flow packet models"
    );
    assert!(
        stream.contains("trojan::TrojanUdpFlowIo")
            && stream.contains(".establish_with_resume(")
            && stream.contains(".write_stream_packet(&mut send_stream, &packet)")
            && stream.contains(".read_stream_packet(&mut recv_stream)")
            && !stream.contains("trojan::establish_udp_packet_tunnel"),
        "Trojan UDP packet stream should call the protocols/trojan flow I/O stream helpers"
    );
}

#[test]
fn trojan_udp_send_orchestration_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/trojan_manager.rs");
    let send = manifest_dir().join("src/protocol_runtime/udp/trojan_manager/send.rs");

    for forbidden in [
        "async fn send(",
        "fn send_relay",
        "send_existing(",
        "send_relay_existing(",
        "trojan_relay_upstream",
        "trojan_establish",
        "trojan_relay_establish",
        "UdpFlowContext",
        "UdpPacketRef",
    ] {
        assert!(
            !manager.contains(forbidden),
            "trojan_manager.rs should keep send orchestration in trojan_manager/send.rs; found `{forbidden}`"
        );
    }
    assert!(
        send.exists(),
        "Trojan UDP send orchestration should live in trojan_manager/send.rs"
    );
}

#[test]
fn mieru_udp_packet_stream_tasks_live_outside_manager() {
    let manager = read("src/protocol_runtime/udp/mieru_manager.rs");
    let stream = read("src/protocol_runtime/udp/mieru_manager/stream.rs");
    let socket = manifest_dir().join("src/protocol_runtime/udp/mieru_manager/socket.rs");
    let protocol_outbound = fs::read_to_string(repo_root().join("protocols/mieru/src/outbound.rs"))
        .expect("read mieru protocol outbound source");

    for forbidden in [
        "tokio::io::split",
        "encrypt_client_data(&payload)",
        "decrypt_server_data_with_consumed(&raw)",
        "decode_udp_flow_packet",
        "encode_udp_flow_packet",
        "parse_udp_packet",
        "AsyncReadExt",
        "AsyncWriteExt",
    ] {
        assert!(
            !manager.contains(forbidden),
            "mieru_manager.rs should keep packet stream task details in mieru_manager/stream.rs; found `{forbidden}`"
        );
    }
    for forbidden in [
        "packet.encode_with",
        "push_encrypted_response",
        "next_packet",
        "AsyncReadExt",
        "AsyncWriteExt",
    ] {
        assert!(
            !stream.contains(forbidden),
            "mieru_manager/stream.rs should keep only runtime pump glue and delegate protocol I/O to protocols/mieru; found `{forbidden}`"
        );
    }
    assert!(
        socket.exists(),
        "Mieru UDP stream half AsyncSocket adapters should live in mieru_manager/socket.rs"
    );
    assert!(
        stream.contains("MieruUdpFlowIo")
            && stream.contains("io.write_packet(&mut write_stream, &packet)")
            && stream.contains("io.read_packets(&mut read_stream, &mut scratch)")
            && stream.contains("packet.into_parts()")
            && !stream.contains("encrypt_packet(")
            && !stream.contains("MieruOutbound")
            && protocol_outbound.contains("pub async fn write_packet")
            && protocol_outbound.contains("pub async fn read_packets"),
        "Mieru UDP packet stream tasks should delegate packet stream I/O operations to protocols/mieru"
    );
}

#[test]
fn h2_udp_datagram_codec_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/h2_manager.rs");
    let stream = read("src/protocol_runtime/udp/h2_manager/stream.rs");
    let manager_send = read("src/protocol_runtime/udp/h2_manager/send.rs");
    let manager_model = read("src/protocol_runtime/udp/h2_manager/model.rs");
    let adapter = read("src/adapters/hysteria2/udp.rs");
    let snapshot = read("src/protocol_runtime/udp/flow_snapshot.rs");
    let forward = read("src/protocol_runtime/udp/state/forward/hysteria2.rs");
    let protocol_udp = fs::read_to_string(repo_root().join("protocols/hysteria2/src/udp.rs"))
        .expect("read hysteria2 protocol udp source");

    for forbidden in [
        "UdpDatagramFraming",
        "Hysteria2UdpPacketTarget",
        "Hysteria2UdpPacket",
    ] {
        assert!(
            !manager.contains(forbidden),
            "h2_manager.rs should not own Hysteria2 datagram codec details; found `{forbidden}`"
        );
        assert!(
            !stream.contains(forbidden),
            "h2_manager/stream.rs should delegate Hysteria2 packet codec details to protocols/hysteria2; found `{forbidden}`"
        );
    }
    assert!(
        !manifest_dir()
            .join("src/protocol_runtime/udp/h2_manager/codec.rs")
            .exists(),
        "Hysteria2 UDP manager should not keep a proxy-owned codec module"
    );
    assert!(
        !adapter.contains("hysteria2::udp_flow_codec")
            && adapter.contains("Hysteria2UdpPacketPathConfig")
            && protocol_udp.contains("pub fn udp_flow_codec(")
            && protocol_udp.contains("struct Hysteria2UdpPacketPathConfig")
            && protocol_udp.contains("impl DatagramCodec<Address> for Hysteria2DatagramCodec")
            && protocol_udp.contains("pub fn encode_packet(")
            && protocol_udp.contains("pub fn decode_packet(&self"),
        "Hysteria2 adapter and UDP manager should consume protocol-owned UDP flow packet helpers"
    );
    let h2_entry_model = manager_model
        .split("pub(super) struct H2Entry")
        .nth(1)
        .expect("H2Entry model should exist")
        .split("pub(crate) struct H2SendExisting")
        .next()
        .expect("H2SendExisting should follow H2Entry");
    assert!(
        h2_entry_model.contains("mpsc::Sender<hysteria2::Hysteria2UdpFlowPacket>")
            && !h2_entry_model.contains("resume: hysteria2::Hysteria2UdpFlowResume")
            && manager_send.contains("Hysteria2UdpFlowPacket::from_parts")
            && stream.contains("Hysteria2UdpFlowPacket::from_parts")
            && stream.contains("initial_packet.encode_with(&resume)")
            && stream.contains("packet.encode_with(&resume)")
            && stream.contains("resume.decode_flow_packet(&data)")
            && !manager_send.contains(".encode_packet(")
            && !stream.contains(".encode_packet(")
            && !stream.contains(".decode_packet(")
            && !stream.contains("mpsc::Sender<Vec<u8>>"),
        "Hysteria2 UDP manager should carry protocol-owned flow packet models and keep encode/decode operations behind protocol helpers"
    );
    assert!(
        adapter.contains("Hysteria2UdpFlowResume::new")
            && protocol_udp.contains("struct Hysteria2UdpFlowResume")
            && protocol_udp.contains("pub fn peer_config(&self)")
            && protocol_udp.contains("struct Hysteria2UdpLeafKey")
            && protocol_udp.contains("pub fn codec(&self)")
            && protocol_udp.contains("pub fn client_fingerprint(&self) -> Option<&str>"),
        "Hysteria2 adapter should build an opaque protocol-owned UDP flow resume descriptor"
    );
    let resume_enum = snapshot
        .split("pub(crate) enum ProtocolUdpFlowResume")
        .nth(1)
        .expect("ProtocolUdpFlowResume enum should exist")
        .split("impl ProtocolUdpFlowSnapshot")
        .next()
        .expect("ProtocolUdpFlowSnapshot impl should follow ProtocolUdpFlowResume");
    assert!(
        snapshot.contains("resume: ProtocolUdpFlowResume")
            && snapshot.contains("Hysteria2(hysteria2::Hysteria2UdpFlowResume)")
            && !resume_enum.contains("password: String")
            && !resume_enum.contains("client_fingerprint: Option<String>"),
        "Hysteria2 protocol UDP flow snapshot should carry only the unified opaque resume wrapper"
    );
    assert!(
        forward.contains("resume: existing.resume.clone()")
            && !forward.contains("existing.resume.password()")
            && !forward.contains("existing.resume.client_fingerprint()")
            && !forward.contains("existing.resume.codec()")
            && !forward.contains("hysteria2::udp_flow_codec")
            && !forward.contains("password: &'a str")
            && !forward.contains("client_fingerprint: Option<&'a str>"),
        "existing Hysteria2 UDP flow forwarding should pass the opaque resume descriptor without unpacking auth or codec state"
    );
    for forbidden in [
        "request.resume.password()",
        "request.resume.client_fingerprint()",
        "password: String",
        "client_fingerprint: Option<String>",
        "peer.password",
        "peer.client_fingerprint",
    ] {
        assert!(
            !manager_send.contains(forbidden)
                && !manager_model.contains(forbidden)
                && !stream.contains(forbidden),
            "Hysteria2 UDP manager should use protocol-owned peer config/key instead of unpacking `{forbidden}`"
        );
    }
}

#[test]
fn h2_packet_path_carrier_uses_protocol_built_codec() {
    let adapter = read("src/adapters/hysteria2/udp.rs");
    let carrier =
        read("src/protocol_runtime/udp/packet_path_chain/carriers/quic_datagram_carrier.rs");
    let protocol_udp = fs::read_to_string(repo_root().join("protocols/hysteria2/src/udp.rs"))
        .expect("read hysteria2 protocol udp source");

    assert!(
        !adapter.contains("hysteria2::udp_flow_codec")
            && !adapter.contains("hysteria2::udp_cache_key")
            && adapter.contains("Hysteria2UdpPacketPathConfig"),
        "Hysteria2 adapter should request protocol-built packet-path cache identity and codec through a protocol config helper"
    );
    assert!(
        protocol_udp.contains("pub fn udp_flow_codec(")
            && protocol_udp.contains("struct Hysteria2UdpPacketPathConfig")
            && protocol_udp.contains("impl DatagramCodec<Address> for Hysteria2DatagramCodec"),
        "protocols/hysteria2 should own Hysteria2 UDP flow codec construction"
    );
    for forbidden in [
        "hysteria2::build_udp_datagram",
        "hysteria2::parse_udp_datagram",
        "hysteria2::encode_udp_flow_packet",
        "hysteria2::decode_udp_flow_packet",
        "Hysteria2UdpPacketTarget",
        "Hysteria2Connector",
        "connect_raw",
        "client_fingerprint",
        "password: &str",
    ] {
        assert!(
            !carrier.contains(forbidden),
            "QUIC datagram packet-path carrier should consume adapter-provided connection/codec objects instead of naming protocol details; found `{forbidden}`"
        );
    }
    assert!(
        carrier.contains("Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>")
            && carrier.contains(".codec")
            && carrier.contains("conn: Arc<quinn::Connection>")
            && adapter.contains("Hysteria2Connector")
            && adapter.contains("connect_raw"),
        "Hysteria2 adapter should own protocol-specific QUIC connection setup while the carrier keeps only connection lifecycle and codec use"
    );
}

#[test]
fn h2_udp_response_bridge_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/h2_manager.rs");
    let bridge = manifest_dir().join("src/protocol_runtime/udp/h2_manager/bridge.rs");

    for forbidden in [
        "type RecvItem",
        "broadcast::channel",
        "recv_tx.subscribe",
        "h2 upstream closed",
    ] {
        assert!(
            !manager.contains(forbidden),
            "h2_manager.rs should keep response bridge details in h2_manager/bridge.rs; found `{forbidden}`"
        );
    }
    assert!(
        bridge.exists(),
        "Hysteria2 UDP response bridge should live in h2_manager/bridge.rs"
    );
}

#[test]
fn h2_udp_packet_stream_tasks_live_outside_manager() {
    let manager = read("src/protocol_runtime/udp/h2_manager.rs");
    let stream = manifest_dir().join("src/protocol_runtime/udp/h2_manager/stream.rs");

    for forbidden in [
        "Hysteria2Connector",
        "connect_raw",
        "send_datagram",
        "read_datagram",
        "tokio::spawn",
    ] {
        assert!(
            !manager.contains(forbidden),
            "h2_manager.rs should keep QUIC packet stream task details in h2_manager/stream.rs; found `{forbidden}`"
        );
    }
    assert!(
        stream.exists(),
        "Hysteria2 UDP packet stream tasks should live in h2_manager/stream.rs"
    );
}

#[test]
fn h2_udp_state_model_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/h2_manager.rs");
    let model = manifest_dir().join("src/protocol_runtime/udp/h2_manager/model.rs");

    for forbidden in ["struct H2Entry", "struct H2SendExisting", "struct H2Key"] {
        assert!(
            !manager.contains(forbidden),
            "h2_manager.rs should keep state/request models in h2_manager/model.rs; found `{forbidden}`"
        );
    }
    assert!(
        model.exists(),
        "Hysteria2 UDP state/request models should live in h2_manager/model.rs"
    );
}

#[test]
fn h2_udp_model_details_live_outside_manager_root() {
    let manager = read("src/protocol_runtime/udp/h2_manager.rs");
    let model = read("src/protocol_runtime/udp/h2_manager/model.rs");

    for forbidden in ["struct H2Entry", "struct H2SendExisting", "struct H2Key"] {
        assert!(
            !manager.contains(forbidden),
            "h2_manager.rs should keep model details in h2_manager/model.rs; found `{forbidden}`"
        );
    }

    for required in ["struct H2Entry", "struct H2SendExisting", "struct H2Key"] {
        assert!(
            model.contains(required),
            "h2_manager model details should live in h2_manager/model.rs; missing `{required}`"
        );
    }
}

#[test]
fn h2_udp_send_orchestration_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/h2_manager.rs");
    let send = manifest_dir().join("src/protocol_runtime/udp/h2_manager/send.rs");

    for forbidden in [
        "async fn send(",
        "pub(crate) async fn send_existing",
        "H2Key::from_peer",
        "h2_udp_packet",
        "h2_establish",
    ] {
        assert!(
            !manager.contains(forbidden),
            "h2_manager.rs should keep send orchestration in h2_manager/send.rs; found `{forbidden}`"
        );
    }
    assert!(
        send.exists(),
        "Hysteria2 UDP send orchestration should live in h2_manager/send.rs"
    );
}

#[test]
fn h2_udp_establish_logic_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/h2_manager.rs");
    let establish = manifest_dir().join("src/protocol_runtime/udp/h2_manager/establish.rs");

    for forbidden in [
        "async fn establish",
        "stream::establish",
        "spawn_response_bridge",
    ] {
        assert!(
            !manager.contains(forbidden),
            "h2_manager.rs should keep establish glue in h2_manager/establish.rs; found `{forbidden}`"
        );
    }
    assert!(
        establish.exists(),
        "Hysteria2 UDP establish glue should live in h2_manager/establish.rs"
    );
}

#[test]
fn shadowsocks_udp_datagram_codec_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/ss_manager.rs");
    let adapter = read("src/adapters/shadowsocks/udp.rs");
    let socket = read("src/protocol_runtime/udp/ss_manager/socket.rs");
    let protocol_outbound =
        fs::read_to_string(repo_root().join("protocols/shadowsocks/src/outbound.rs"))
            .expect("read shadowsocks protocol outbound source");

    for forbidden in [
        "UdpDatagramFraming",
        "ShadowsocksUdpPacketTarget",
        "ShadowsocksUdpDecodeContext",
        "ShadowsocksUdpPacket",
    ] {
        assert!(
            !manager.contains(forbidden),
            "ss_manager.rs should not own Shadowsocks datagram codec details; found `{forbidden}`"
        );
        assert!(
            !socket.contains(forbidden),
            "ss_manager/socket.rs should delegate Shadowsocks packet codec details to protocols/shadowsocks; found `{forbidden}`"
        );
    }
    assert!(
        !manifest_dir()
            .join("src/protocol_runtime/udp/ss_manager/codec.rs")
            .exists(),
        "Shadowsocks UDP manager should not keep a proxy-owned codec module"
    );
    assert!(
        !adapter.contains("shadowsocks::udp_flow_codec")
            && adapter.contains("ShadowsocksUdpFlowResume::from_config")
            && protocol_outbound.contains("pub fn udp_flow_codec(")
            && protocol_outbound.contains("pub fn from_config(")
            && protocol_outbound
                .contains("impl DatagramCodec<Address> for ShadowsocksDatagramCodec")
            && protocol_outbound.contains("struct ShadowsocksUdpFlowPacket")
            && protocol_outbound.contains("pub fn encode_with(")
            && protocol_outbound.contains("pub fn decode_flow_packet(&self"),
        "Shadowsocks adapter and UDP manager should consume protocol-owned UDP flow packet helpers"
    );
    for forbidden in [".encode_packet(", ".decode_packet("] {
        assert!(
            !manager.contains(forbidden) && !socket.contains(forbidden),
            "Shadowsocks UDP manager glue should not call raw protocol packet codec operations directly; found `{forbidden}`"
        );
    }
}

#[test]
fn shadowsocks_udp_response_bridge_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/ss_manager.rs");
    let bridge = manifest_dir().join("src/protocol_runtime/udp/ss_manager/bridge.rs");

    for forbidden in [
        "oneshot::channel",
        "VecDeque",
        "struct SsResponseWaiter",
        "fn remove_waiter",
    ] {
        assert!(
            !manager.contains(forbidden),
            "ss_manager.rs should keep response waiter bridge details in ss_manager/bridge.rs; found `{forbidden}`"
        );
    }
    assert!(
        bridge.exists(),
        "Shadowsocks UDP response bridge should live in ss_manager/bridge.rs"
    );
}

#[test]
fn shadowsocks_udp_socket_runtime_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/ss_manager.rs");
    let socket = manifest_dir().join("src/protocol_runtime/udp/ss_manager/socket.rs");

    for forbidden in [
        "UdpSocket::bind",
        "from_std",
        "fn recv_loop",
        "tokio::spawn(Self::recv_loop",
        "shadowsocks udp recv loop stopped",
    ] {
        assert!(
            !manager.contains(forbidden),
            "ss_manager.rs should keep socket runtime details in ss_manager/socket.rs; found `{forbidden}`"
        );
    }
    assert!(
        socket.exists(),
        "Shadowsocks UDP socket runtime should live in ss_manager/socket.rs"
    );
}

#[test]
fn shadowsocks_udp_state_model_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/ss_manager.rs");
    let model = manifest_dir().join("src/protocol_runtime/udp/ss_manager/model.rs");

    for forbidden in [
        "struct SsUpstream",
        "struct SsSendExisting",
        "struct SsKey",
        "format!(\"{cipher_kind:?}\")",
    ] {
        assert!(
            !manager.contains(forbidden),
            "ss_manager.rs should keep state/request models in ss_manager/model.rs; found `{forbidden}`"
        );
    }
    assert!(
        model.exists(),
        "Shadowsocks UDP state/request models should live in ss_manager/model.rs"
    );
}

#[test]
fn shadowsocks_udp_flow_cipher_is_adapter_parsed() {
    let adapter = read("src/adapters/shadowsocks/udp.rs");
    let flows = read("src/protocol_runtime/udp/flows.rs");
    let peer = read("src/protocol_runtime/udp/peer.rs");
    let manager = read("src/protocol_runtime/udp/ss_manager.rs");
    let model = read("src/protocol_runtime/udp/ss_manager/model.rs");
    let snapshot = read("src/protocol_runtime/udp/flow_snapshot.rs");
    let forward = read("src/protocol_runtime/udp/state/forward/shadowsocks.rs");
    let protocol_outbound =
        fs::read_to_string(repo_root().join("protocols/shadowsocks/src/outbound.rs"))
            .expect("read shadowsocks protocol outbound source");

    assert!(
        !adapter.contains("CipherKind::from_str")
            && adapter.contains("ShadowsocksUdpFlowResume::from_config")
            && protocol_outbound.contains("pub fn parse_udp_cipher("),
        "Shadowsocks UDP adapter should ask protocols/shadowsocks to parse ordinary UDP flow cipher config"
    );
    for source in [&manager, &model] {
        assert!(
            !source.contains("CipherKind::from_str")
                && !source.contains("cipher: shadowsocks::CipherKind")
                && !source.contains("password: &'a str"),
            "Shadowsocks UDP manager state should receive adapter-built codec/cache identity instead of raw cipher/password details"
        );
    }
    let shadowsocks_flow_model = flows
        .split("#[cfg(feature = \"mieru\")]")
        .next()
        .expect("Shadowsocks UDP flow model should appear before Mieru");
    let shadowsocks_peer_model = peer
        .split("/// Hysteria2 UDP peer parameters.")
        .next()
        .expect("Shadowsocks UDP peer model should appear before Hysteria2");
    assert!(
        !shadowsocks_flow_model.contains("cipher: shadowsocks::CipherKind")
            && !shadowsocks_flow_model.contains("password: &'a str")
            && !shadowsocks_flow_model.contains("cache_key: String")
            && !shadowsocks_flow_model.contains("DatagramCodec")
            && shadowsocks_flow_model.contains("resume: ProtocolUdpFlowResume"),
        "ordinary Shadowsocks UDP flow model should carry only the unified resume descriptor"
    );
    assert!(
        !shadowsocks_peer_model.contains("cipher: shadowsocks::CipherKind")
            && !shadowsocks_peer_model.contains("password: &'a str")
            && !shadowsocks_peer_model.contains("cache_key: &'a str")
            && shadowsocks_peer_model.contains("leaf_key: shadowsocks::ShadowsocksUdpLeafKey"),
        "ordinary Shadowsocks UDP peer model should carry only protocol-owned opaque cache identity"
    );
    assert!(
        adapter.contains("ShadowsocksUdpFlowResume::from_config")
            && !adapter.contains("ShadowsocksUdpFlowResume::new")
            && protocol_outbound.contains("struct ShadowsocksUdpFlowResume")
            && protocol_outbound.contains("pub fn leaf_cache_key(&self)")
            && protocol_outbound.contains("struct ShadowsocksUdpLeafKey")
            && protocol_outbound.contains("pub fn from_config(")
            && protocol_outbound.contains("pub fn codec(&self)")
            && protocol_outbound.contains("pub fn cache_key(&self) -> &str"),
        "Shadowsocks adapter should build an opaque protocol-owned UDP flow resume descriptor"
    );
    assert!(
        snapshot.contains("resume: ProtocolUdpFlowResume")
            && snapshot.contains("Shadowsocks(shadowsocks::ShadowsocksUdpFlowResume)")
            && !snapshot.contains("cipher_kind: shadowsocks::CipherKind")
            && !snapshot.contains("datagram_cache_key: String"),
        "Shadowsocks protocol UDP flow snapshot should carry only the unified opaque resume wrapper"
    );
    assert!(
        forward.contains("resume: existing.resume.clone()")
            && !forward.contains("existing.resume.cache_key()")
            && !forward.contains("existing.resume.codec()")
            && !forward.contains("shadowsocks::udp_flow_codec")
            && !forward.contains("password: &'a str")
            && !forward.contains("cipher_kind: shadowsocks::CipherKind")
            && !forward.contains("datagram_cache_key: &'a str"),
        "existing Shadowsocks UDP flow forwarding should pass the opaque resume descriptor without unpacking cache identity or codec state"
    );
    let start = read("src/protocol_runtime/udp/start/datagram.rs");
    assert!(
        start.contains("ProtocolUdpFlowResume::Shadowsocks(resume)")
            && start.contains("resume: resume.clone()")
            && !start.contains("resume.cache_key()")
            && !start.contains("resume.codec()"),
        "new Shadowsocks UDP flow start should pass the unified resume descriptor without unpacking cache identity or codec state"
    );
    let manager = read("src/protocol_runtime/udp/ss_manager.rs");
    let manager_model = read("src/protocol_runtime/udp/ss_manager/model.rs");
    for forbidden in [
        "request.resume.cache_key()",
        "request.resume.codec()",
        "cache_key: String",
        "cache_key: &str",
        "SsKey::new(server",
    ] {
        assert!(
            !manager.contains(forbidden) && !manager_model.contains(forbidden),
            "Shadowsocks UDP manager should use protocol-owned leaf key instead of unpacking `{forbidden}`"
        );
    }
}

#[test]
fn shadowsocks_packet_path_cipher_is_adapter_parsed() {
    let adapter = read("src/adapters/shadowsocks/udp.rs");
    let protocol_outbound = manifest_dir()
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .join("protocols/shadowsocks/src/outbound.rs");
    let protocol_outbound =
        fs::read_to_string(protocol_outbound).expect("read shadowsocks protocol outbound source");
    let carrier = read("src/protocol_runtime/udp/packet_path_chain/carriers.rs");
    let udp_socket_carrier =
        read("src/protocol_runtime/udp/packet_path_chain/carriers/udp_socket_carrier.rs");
    let entry = read("src/protocol_runtime/udp/packet_path_chain/entry.rs");
    let traits = read("src/protocol_runtime/udp/packet_path_traits/carrier.rs");
    let key = read("src/protocol_runtime/udp/packet_path_chain/key.rs");
    let outbound = read("src/runtime/udp_flow/outbound.rs");
    let carrier_snapshot = read("src/protocol_runtime/udp/packet_path_snapshot.rs");
    let snapshot = read("src/protocol_runtime/udp/packet_path_chain/snapshot.rs");
    let forward = read("src/protocol_runtime/udp/state/forward/shadowsocks.rs");

    assert!(
        !adapter.contains("CipherKind::from_str") && adapter.contains("ShadowsocksUdpFlowResume::from_config"),
        "Shadowsocks adapter should ask protocols/shadowsocks to parse packet-path carrier/datagram cipher config"
    );
    for forbidden in ["ShadowsocksDatagramCodec", "shadowsocks::"] {
        assert!(
            !udp_socket_carrier.contains(forbidden),
            "UDP socket packet-path carrier should consume an adapter-provided codec instead of naming protocol framing; found `{forbidden}`"
        );
    }
    assert!(
        !carrier_snapshot.contains("ShadowsocksDatagramCodec")
            && !carrier_snapshot.contains("shadowsocks::udp_datagram_codec")
            && !adapter.contains("shadowsocks::udp_datagram_codec")
            && !adapter.contains("resume.codec()")
            && adapter.contains("resume.packet_path_codec()"),
        "Shadowsocks adapter should request protocol-built packet-path codecs through explicit protocol packet-path helpers"
    );
    assert!(
        !adapter.contains("shadowsocks::udp_flow_codec")
            && protocol_outbound.contains("fn udp_flow_codec(")
            && protocol_outbound.contains("pub fn from_config("),
        "Shadowsocks adapter should request protocol-built packet-path codecs through opaque resume config"
    );
    assert!(
        !udp_socket_carrier.contains("shadowsocks::encode_udp_flow_packet")
            && !udp_socket_carrier.contains("shadowsocks::decode_udp_flow_packet"),
        "UDP socket packet-path carrier should not call flow-specific protocols/shadowsocks helpers directly"
    );
    for source in [&carrier, &entry] {
        assert!(
            !source.contains("CipherKind::from_str"),
            "packet-path chain should receive adapter-parsed Shadowsocks cipher values"
        );
    }
    assert!(
        !traits.contains("password: &'a str")
            && !traits.contains("cipher_kind: shadowsocks::CipherKind")
            && traits.contains("struct UdpDatagramDescriptor")
            && traits.contains("cache_key: String")
            && traits.contains("descriptor: UdpDatagramDescriptor<'a>")
            && !traits.contains("ProtocolUdpFlowSnapshot")
            && traits.contains("codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>"),
        "UdpDatagramSource should contain only neutral descriptor identity and adapter-provided packet-path datagram codec"
    );
    assert!(
        !adapter.contains("shadowsocks::udp_cache_key")
            && !adapter.contains("resume.cache_key()")
            && adapter.contains("resume.packet_path_cache_key()"),
        "Shadowsocks adapter should receive opaque packet-path cache keys from protocols/shadowsocks resume config"
    );
    assert!(
        protocol_outbound.contains("fn udp_cache_key(")
            && protocol_outbound.contains("pub fn packet_path_cache_key(&self)")
            && protocol_outbound.contains("pub fn packet_path_codec(&self)"),
        "protocols/shadowsocks should own Shadowsocks cache identity and packet-path codec construction"
    );
    for source in [
        &traits,
        &key,
        &outbound,
        &carrier_snapshot,
        &snapshot,
        &forward,
    ] {
        assert!(
            !source.contains("datagram_cipher") && !source.contains("cipher: &'a str"),
            "packet-path runtime should not carry raw Shadowsocks cipher strings for cache identity"
        );
    }
    assert!(
        !carrier_snapshot.contains("cipher: String")
            && !carrier_snapshot.contains("enum UdpPacketPathCarrier"),
        "packet-path carrier snapshots should use neutral adapter-built cache keys instead of protocol-specific payload fields"
    );
}

#[test]
fn adapters_do_not_own_udp_packet_path_cache_key_formats() {
    for source in [
        "src/adapters/socks5/udp.rs",
        "src/adapters/shadowsocks/udp.rs",
        "src/adapters/hysteria2/udp.rs",
    ] {
        let content = read(source);
        for forbidden in ["socks5|", "shadowsocks|", "hysteria2|", "|auth:", "|fp:"] {
            assert!(
                !content.contains(forbidden),
                "{source} should ask the owning protocol/runtime helper for packet-path cache identity instead of formatting `{forbidden}`"
            );
        }
    }

    let udp_root = read("src/protocol_runtime/udp/mod.rs");
    let packet_path_snapshot = read("src/protocol_runtime/udp/packet_path_snapshot.rs");
    let socks5_shared = manifest_dir()
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .join("protocols/socks5/src/shared.rs");
    let socks5_shared =
        fs::read_to_string(socks5_shared).expect("read socks5 protocol shared source");
    assert!(
        !udp_root.contains("mod cache_key")
            && !packet_path_snapshot.contains("socks5_udp_cache_key"),
        "protocol_runtime::udp should not own packet-path cache identity helpers"
    );
    assert!(
        socks5_shared.contains("fn udp_cache_key(") && socks5_shared.contains("socks5|"),
        "protocols/socks5 should own SOCKS5 cache identity construction"
    );
    let socks5_adapter = read("src/adapters/socks5/udp.rs");
    assert!(
        !socks5_adapter.contains("socks5::udp_cache_key")
            && socks5_adapter.contains("Socks5UdpPacketPathConfig"),
        "SOCKS5 adapter should request packet-path cache identity through a protocol-owned config helper"
    );
}

#[test]
fn adapters_do_not_construct_udp_packet_path_snapshots_directly() {
    for source in [
        "src/adapters/socks5/udp.rs",
        "src/adapters/shadowsocks/udp.rs",
        "src/adapters/hysteria2/udp.rs",
    ] {
        let content = read(source);
        for forbidden in [
            "PacketPathCarrierDescriptor {",
            "UdpDatagramSource {",
            "UdpPacketPathCarrier::Socks5",
            "UdpPacketPathCarrier::Shadowsocks",
            "UdpPacketPathCarrier::Hysteria2",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should use protocol_runtime::udp packet-path constructors instead of `{forbidden}`"
            );
        }
    }

    let snapshot = read("src/protocol_runtime/udp/packet_path_snapshot.rs");
    let root = read("src/protocol_runtime/udp/mod.rs");
    for required in ["packet_path_carrier_descriptor", "udp_datagram_source"] {
        assert!(
            snapshot.contains(required),
            "protocol_runtime::udp packet-path snapshot module should own neutral constructor `{required}`"
        );
    }
    for forbidden in [
        "socks5_packet_path_carrier_descriptor",
        "shadowsocks_packet_path_carrier_descriptor",
        "shadowsocks_udp_datagram_source",
        "shadowsocks_packet_path_flow_snapshot",
        "hysteria2_packet_path_carrier_descriptor",
        "socks5::",
        "shadowsocks::",
        "hysteria2::",
        "socks5_packet_path_carrier_snapshot",
        "shadowsocks_packet_path_carrier_snapshot",
        "hysteria2_packet_path_carrier_snapshot",
        "UdpPacketPathCarrier::Socks5",
        "UdpPacketPathCarrier::Shadowsocks",
        "UdpPacketPathCarrier::Hysteria2",
    ] {
        assert!(
            !snapshot.contains(forbidden),
            "packet-path snapshot module should not retain protocol-named carrier snapshot storage `{forbidden}`"
        );
    }
    assert!(
        !snapshot.contains("ProtocolUdpFlowSnapshot::shadowsocks(")
            && !snapshot.contains("ProtocolUdpFlowSnapshot"),
        "packet-path snapshot helpers should not construct or name protocol flow snapshots"
    );
    assert!(
        !snapshot.contains("protocol_snapshot:"),
        "packet-path datagram source should not carry the protocol flow snapshot"
    );
    assert!(
        !snapshot.contains("ProtocolUdpFlowSnapshot::Shadowsocks {"),
        "packet-path flow snapshot helper should not construct Shadowsocks snapshot fields directly"
    );
    for forbidden in [
        "pub(crate) use packet_path_snapshot::{",
        "socks5_packet_path_carrier_descriptor",
        "shadowsocks_packet_path_carrier_descriptor",
        "shadowsocks_udp_datagram_source",
        "shadowsocks_packet_path_flow_snapshot",
        "hysteria2_packet_path_carrier_descriptor",
        "pub(crate) use packet_path_chain::build_shadowsocks_packet_path",
        "pub(crate) use packet_path_chain::build_hysteria2_packet_path",
    ] {
        assert!(
            !root.contains(forbidden),
            "protocol_runtime::udp root should not re-export protocol-named packet-path helper `{forbidden}`"
        );
    }
    for source in [
        "src/adapters/socks5/udp.rs",
        "src/adapters/shadowsocks/udp.rs",
        "src/adapters/hysteria2/udp.rs",
    ] {
        let content = read(source);
        assert!(
            content.contains("crate::protocol_runtime::udp::packet_path_snapshot::"),
            "{source} should call packet-path snapshot helpers through the explicit snapshot module"
        );
    }
    for source in [
        "src/adapters/shadowsocks/udp.rs",
        "src/adapters/hysteria2/udp.rs",
    ] {
        let content = read(source);
        assert!(
            content.contains("crate::protocol_runtime::udp::packet_path_chain::"),
            "{source} should call packet-path carrier builders through the explicit chain module"
        );
    }
}

#[test]
fn shadowsocks_udp_entry_cache_lives_outside_manager() {
    let manager = read("src/protocol_runtime/udp/ss_manager.rs");
    let entry = manifest_dir().join("src/protocol_runtime/udp/ss_manager/entry.rs");

    for forbidden in [
        "fn ensure_entry",
        "SsKey::new",
        "socket::bind_for_target",
        "BridgeWaiters::new",
        "socket::spawn_recv_loop",
    ] {
        assert!(
            !manager.contains(forbidden),
            "ss_manager.rs should keep entry/cache construction in ss_manager/entry.rs; found `{forbidden}`"
        );
    }
    assert!(
        entry.exists(),
        "Shadowsocks UDP entry/cache construction should live in ss_manager/entry.rs"
    );
}

#[test]
fn adapters_do_not_reach_into_udp_dispatch_manager_fields() {
    for path in rust_sources_under("src/adapters") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        for forbidden in [
            ".protocol_state",
            ".ss_manager",
            ".h2_manager",
            ".trojan_manager",
            ".mieru_manager",
            ".vless_manager",
            ".vmess_manager",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should not reach into udp-dispatch manager field `{forbidden}`"
            );
        }
    }
}

#[test]
fn udp_adapters_use_dispatch_facades_for_protocol_state() {
    for path in rust_sources_under("src/adapters") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        assert!(
            !content.contains("protocol_parts()"),
            "{source} should ask UdpDispatch facades to start protocol state instead of borrowing protocol_parts()"
        );
        assert!(
            !content.contains("ProtocolUdpFlowSnapshot"),
            "{source} should ask UdpDispatch facades to describe protocol UDP flow snapshots"
        );
        if source != "src/adapters/direct/udp.rs" {
            assert!(
                !content.contains("FlowStartResult::Flow"),
                "{source} should let UdpDispatch facades build tracked protocol UDP flow results"
            );
        }
    }

    for (source, request, start, flow_start) in [
        (
            "src/runtime/udp_dispatch/hysteria2_flow.rs",
            "Hysteria2DatagramSend",
            "start_hysteria2_udp_flow",
            "start_hysteria2_datagram_flow",
        ),
        (
            "src/runtime/udp_dispatch/mieru_flow.rs",
            "MieruDatagramSend",
            "start_mieru_udp_flow",
            "start_mieru_datagram_flow",
        ),
        (
            "src/runtime/udp_dispatch/shadowsocks_flow.rs",
            "ShadowsocksDatagramSend",
            "start_shadowsocks_udp_flow",
            "start_shadowsocks_datagram_flow",
        ),
        (
            "src/runtime/udp_dispatch/socks5_flow.rs",
            "Socks5RelaySend",
            "send_socks5_packet",
            "start_socks5_relay_flow",
        ),
        (
            "src/runtime/udp_dispatch/trojan_flow.rs",
            "TrojanDatagramSend",
            "start_trojan_udp_flow",
            "start_trojan_datagram_flow",
        ),
        (
            "src/runtime/udp_dispatch/vless_flow.rs",
            "VlessDatagramSend",
            "start_vless_udp_flow",
            "send_vless_datagram",
        ),
        (
            "src/runtime/udp_dispatch/vmess_flow.rs",
            "VmessDatagramSend",
            "start_vmess_udp_flow",
            "send_vmess_datagram",
        ),
    ] {
        let facade = read(source);
        for required in [request, start, flow_start] {
            assert!(
                facade.contains(required),
                "{source} should own dispatch facade detail `{required}`"
            );
        }
        if source != "src/runtime/udp_dispatch/socks5_flow.rs" {
            assert!(
                facade.contains("&mut self.chain_tasks"),
                "{source} should own chain task bridging for packet/stream UDP flows"
            );
        }
    }

    for source in [
        "src/runtime/udp_dispatch/hysteria2_flow.rs",
        "src/runtime/udp_dispatch/mieru_flow.rs",
        "src/runtime/udp_dispatch/shadowsocks_flow.rs",
        "src/runtime/udp_dispatch/socks5_flow.rs",
        "src/runtime/udp_dispatch/trojan_flow.rs",
    ] {
        let facade = read(source);
        assert!(
            facade.contains("ProtocolUdpFlowSnapshot") && facade.contains("FlowStartResult::Flow"),
            "{source} should own tracked protocol UDP flow result construction"
        );
        for forbidden in [
            "ProtocolUdpFlowSnapshot::Socks5",
            "ProtocolUdpFlowSnapshot::Shadowsocks",
            "ProtocolUdpFlowSnapshot::Hysteria2",
            "ProtocolUdpFlowSnapshot::Trojan",
            "ProtocolUdpFlowSnapshot::Mieru",
        ] {
            assert!(
                !facade.contains(forbidden),
                "{source} should ask protocol_runtime::udp to build protocol snapshots instead of constructing `{forbidden}`"
            );
        }
    }
}

#[test]
fn protocol_udp_flow_snapshot_constructors_live_in_protocol_runtime() {
    let snapshot = read("src/protocol_runtime/udp/flow_snapshot.rs");
    let snapshot_impl = snapshot
        .split("impl ProtocolUdpFlowSnapshot")
        .nth(1)
        .expect("ProtocolUdpFlowSnapshot impl should exist");

    for required in ["pub(crate) fn managed(", "pub(crate) fn resume("] {
        assert!(
            snapshot_impl.contains(required),
            "protocol_runtime::udp::flow_snapshot should own protocol snapshot constructor `{required}`"
        );
    }
    for forbidden in [
        "pub(crate) fn shadowsocks(",
        "pub(crate) fn hysteria2(",
        "pub(crate) fn trojan(",
        "pub(crate) fn mieru(",
        "pub(crate) fn socks5(",
    ] {
        assert!(
            !snapshot_impl.contains(forbidden),
            "protocol_runtime::udp::flow_snapshot should not keep protocol-specific snapshot constructor `{forbidden}`"
        );
    }
    assert!(
        snapshot.contains("Socks5(socks5::Socks5UdpFlowResume)")
            && snapshot.contains("Self::Managed {"),
        "SOCKS5 UDP snapshot state should use the unified ProtocolUdpFlowResume wrapper"
    );
}

#[test]
fn udp_dispatch_does_not_unpack_protocol_flow_resume() {
    for source in [
        "src/runtime/udp_dispatch/hysteria2_flow.rs",
        "src/runtime/udp_dispatch/mieru_flow.rs",
        "src/runtime/udp_dispatch/shadowsocks_flow.rs",
        "src/runtime/udp_dispatch/trojan_flow.rs",
    ] {
        let content = read(source);
        assert!(
            content.contains("resume: ProtocolUdpFlowResume"),
            "{source} should carry the unified protocol UDP flow resume wrapper"
        );
        for forbidden in [
            ".shadowsocks()",
            ".hysteria2()",
            ".trojan()",
            ".mieru()",
            "resume.cache_key()",
            "resume.username()",
            "resume.password()",
            "resume.codec()",
            "codec: std::sync::Arc<dyn DatagramCodec",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should pass ProtocolUdpFlowResume through without unpacking `{forbidden}`"
            );
        }
    }
}

#[test]
fn adapters_do_not_import_protocol_udp_types_through_runtime_dispatch() {
    for path in rust_sources_under("src/adapters") {
        let source = relative(&path);
        let content = fs::read_to_string(&path).expect("read rust source");
        for forbidden in [
            "crate::runtime::udp_dispatch::PacketPathCarrier",
            "crate::runtime::udp_dispatch::PacketPathCarrierDescriptor",
            "crate::runtime::udp_dispatch::UdpDatagramSource",
            "crate::runtime::udp_dispatch::build_socks5_packet_path",
            "crate::runtime::udp_dispatch::build_shadowsocks_packet_path",
            "crate::runtime::udp_dispatch::build_hysteria2_packet_path",
            "crate::runtime::udp_dispatch::ShadowsocksUdpFlow",
            "crate::runtime::udp_dispatch::MieruUdpRelayFlow",
            "crate::runtime::udp_dispatch::VlessUdpFlow",
            "crate::runtime::udp_dispatch::VmessUdpFlow",
        ] {
            assert!(
                !content.contains(forbidden),
                "{source} should import protocol UDP type directly from protocol_runtime, not `{forbidden}`"
            );
        }
    }
}
